package com.hafij.harborfiles

import android.app.Activity
import android.app.Notification
import android.app.NotificationChannel
import android.app.NotificationManager
import android.app.PendingIntent
import android.app.Service
import android.content.BroadcastReceiver
import android.content.Context
import android.content.Intent
import android.content.IntentFilter
import android.content.pm.ServiceInfo
import android.hardware.usb.UsbAccessory
import android.hardware.usb.UsbManager
import android.os.Bundle
import android.os.IBinder
import android.os.ParcelFileDescriptor
import androidx.compose.runtime.mutableStateOf
import java.io.BufferedInputStream
import java.io.DataInputStream
import java.io.FileInputStream
import java.io.FileOutputStream
import java.io.InputStream
import java.io.OutputStream
import java.net.InetAddress
import java.net.ServerSocket
import java.net.Socket
import java.nio.ByteBuffer
import java.util.concurrent.ConcurrentHashMap
import java.util.concurrent.LinkedBlockingQueue
import java.util.concurrent.atomic.AtomicInteger
import kotlin.concurrent.thread

/** The PC on the USB cable, while it's connected. */
data class UsbPc(val name: String, val port: Int, val pin: String) {
    fun server() = Server("usb-cable", "dav", name, "127.0.0.1", port, "harbor")
}

/**
 * The phone end of the USB cable link to Harbor on the PC (src/harbor/files_usb.py).
 *
 * The PC switches the phone into USB accessory mode; Android asks the user to
 * open Harbor Files. Frames on the cable: type(1) id(4) len(4) payload.
 * The phone sends an empty HELLO until the PC answers with its own HELLO
 * ("key=value" lines: PC name and share PIN). Each HTTP connection a
 * player or Harbor makes to 127.0.0.1:port here becomes one stream (OPEN, DATA,
 * CLOSE) that the PC connects to its share server; ACK hands back credit so
 * one paused video can't stall the cable. The loopback listener is reachable
 * from this phone only.
 */
class UsbLink(
    private val inp: InputStream,
    private val out: OutputStream,
    private val onHello: (UsbPc) -> Unit,
    private val onEnd: () -> Unit,
) {
    private class Stream(val sock: Socket) {
        val inbox = LinkedBlockingQueue<ByteArray>()
        val lock = Object()
        var credit = WINDOW.toLong()
        @Volatile var closed = false
    }

    private val streams = ConcurrentHashMap<Int, Stream>()
    private val ids = AtomicInteger(1)
    private var listener: ServerSocket? = null
    @Volatile var alive = false; private set

    fun start() {
        alive = true
        thread(name = "usb-read", isDaemon = true) { reader() }
        // Ask the PC for its HELLO until it answers (it may start reading later).
        thread(name = "usb-hello", isDaemon = true) {
            repeat(30) {
                if (!alive || listener != null) return@thread
                try { send(HELLO, 0) } catch (_: Exception) { stop(); return@thread }
                Thread.sleep(2000)
            }
        }
    }

    private fun send(kind: Int, id: Int, payload: ByteArray = EMPTY, len: Int = payload.size) {
        synchronized(out) { out.write(pad(frame(kind, id, payload, len))); out.flush() }
    }

    private fun safeSend(kind: Int, id: Int, payload: ByteArray = EMPTY) {
        try { send(kind, id, payload) } catch (_: Exception) { stop() }
    }

    private fun reader() {
        try {
            val d = DataInputStream(BufferedInputStream(inp, 16384))
            while (alive) {
                val kind = d.readUnsignedByte()
                val id = d.readInt()
                val len = d.readInt()
                if (len < 0 || len > MAX_PAYLOAD) break
                val payload = ByteArray(len).also { d.readFully(it) }
                when (kind) {
                    HELLO -> hello(payload)
                    DATA -> streams[id]?.inbox?.put(payload)
                    CLOSE -> streams[id]?.inbox?.put(EMPTY_CLOSE)
                    ACK -> streams[id]?.let { s ->
                        if (len == 4) synchronized(s.lock) { s.credit += ByteBuffer.wrap(payload).int.toLong() and 0xffffffffL; s.lock.notifyAll() }
                    }
                    BYE -> break
                }
            }
        } catch (_: Exception) {
        }
        val was = alive
        stop()
        if (was) onEnd()
    }

    private fun hello(payload: ByteArray) {
        val j = String(payload, Charsets.UTF_8).lines().mapNotNull { l ->
            l.indexOf('=').takeIf { it > 0 }?.let { l.substring(0, it) to l.substring(it + 1) }
        }.toMap()
        val pin = j["pin"].orEmpty()
        if (pin.isEmpty() || listener != null) return
        val ss = ServerSocket(0, 16, InetAddress.getByAddress(byteArrayOf(127, 0, 0, 1)))
        listener = ss
        thread(name = "usb-accept", isDaemon = true) {
            while (alive && !ss.isClosed) {
                val c = try { ss.accept() } catch (_: Exception) { break }
                open(c)
            }
        }
        val name = j["name"].orEmpty().ifBlank { "PC" }.take(60)
        onHello(UsbPc(name, ss.localPort, pin))
    }

    private fun open(c: Socket) {
        val id = ids.getAndIncrement()
        val s = Stream(c)
        streams[id] = s
        try { send(OPEN, id) } catch (_: Exception) { stop(); return }
        thread(name = "usb-in", isDaemon = true) { toPlayer(id, s) }
        thread(name = "usb-out", isDaemon = true) { toPc(id, s) }
    }

    /** PC bytes into the local socket, handing credit back as they are written. */
    private fun toPlayer(id: Int, s: Stream) {
        try {
            val o = s.sock.getOutputStream()
            while (true) {
                val b = s.inbox.take()
                if (b === EMPTY_CLOSE) break
                o.write(b); o.flush()
                safeSend(ACK, id, ByteBuffer.allocate(4).putInt(b.size).array())
            }
        } catch (_: Exception) {
        }
        close(id, s)
    }

    /** Local socket bytes to the PC, never more than the PC's credit. */
    private fun toPc(id: Int, s: Stream) {
        try {
            val i = s.sock.getInputStream()
            val buf = ByteArray(MAX_PAYLOAD)
            while (alive && !s.closed) {
                val n: Int
                synchronized(s.lock) {
                    while (s.credit <= 0 && alive && !s.closed) s.lock.wait(1000)
                    n = minOf(s.credit, MAX_PAYLOAD.toLong()).toInt()
                }
                if (s.closed || !alive) break
                val got = i.read(buf, 0, n)
                if (got < 0) break
                synchronized(s.lock) { s.credit -= got }
                send(DATA, id, buf, got)
            }
        } catch (_: Exception) {
        }
        close(id, s)
    }

    private fun close(id: Int, s: Stream) {
        if (streams.remove(id) != null && alive) safeSend(CLOSE, id)
        drop(s)
    }

    private fun drop(s: Stream) {
        synchronized(s.lock) { s.closed = true; s.lock.notifyAll() }
        s.inbox.offer(EMPTY_CLOSE)
        runCatching { s.sock.close() }
    }

    fun stop(bye: Boolean = false) {
        if (!alive) return
        if (bye) runCatching { send(BYE, 0) }
        alive = false
        runCatching { listener?.close() }
        listener = null
        streams.values.forEach { drop(it) }
        streams.clear()
    }

    companion object {
        const val HELLO = 1; const val OPEN = 2; const val DATA = 3; const val CLOSE = 4
        const val ACK = 5; const val BYE = 6; const val NOP = 0
        const val MAX_PAYLOAD = 64 * 1024
        const val WINDOW = 512 * 1024
        private val EMPTY = ByteArray(0)
        private val EMPTY_CLOSE = ByteArray(0)

        fun frame(kind: Int, id: Int, payload: ByteArray = EMPTY, len: Int = payload.size): ByteArray =
            ByteBuffer.allocate(9 + len).put(kind.toByte()).putInt(id).putInt(len).put(payload, 0, len).array()

        /** Never end a USB write on a packet boundary, so the PC's read always ends. */
        fun pad(b: ByteArray): ByteArray = if (b.size % 64 == 0) b + frame(NOP, 0) else b
    }
}

/** Keeps the cable link running in the foreground; shows the PC in Connect. */
class UsbLinkService : Service() {
    private var pfd: ParcelFileDescriptor? = null
    private var link: UsbLink? = null
    private val detached = object : BroadcastReceiver() {
        override fun onReceive(c: Context, i: Intent) { finish() }
    }

    override fun onBind(intent: Intent?): IBinder? = null

    override fun onStartCommand(intent: Intent?, flags: Int, startId: Int): Int {
        if (intent?.action == ACTION_STOP) { link?.stop(bye = true); finish(); return START_NOT_STICKY }
        startForeground(3, notification("Connecting to the PC over USB…"), ServiceInfo.FOREGROUND_SERVICE_TYPE_DATA_SYNC)
        if (link?.alive == true) return START_NOT_STICKY
        val um = getSystemService(UsbManager::class.java)
        val acc = ourAccessory(um)
        val fd = acc?.takeIf { um.hasPermission(it) }?.let { runCatching { um.openAccessory(it) }.getOrNull() }
        if (fd == null) { finish(); return START_NOT_STICKY }
        pfd = fd
        val f = IntentFilter(UsbManager.ACTION_USB_ACCESSORY_DETACHED)
        if (android.os.Build.VERSION.SDK_INT >= 33) registerReceiver(detached, f, RECEIVER_NOT_EXPORTED) else registerReceiver(detached, f)
        link = UsbLink(
            FileInputStream(fd.fileDescriptor), FileOutputStream(fd.fileDescriptor),
            onHello = { pc ->
                state.value = pc
                getSystemService(NotificationManager::class.java).notify(3, notification("Connected to ${pc.name} over USB"))
            },
            onEnd = { finish() },
        ).also { it.start() }
        return START_NOT_STICKY
    }

    private fun finish() {
        state.value = null
        link?.stop(); link = null
        runCatching { unregisterReceiver(detached) }
        runCatching { pfd?.close() }; pfd = null
        stopForeground(STOP_FOREGROUND_REMOVE)
        stopSelf()
    }

    private fun notification(text: String): Notification {
        val nm = getSystemService(NotificationManager::class.java)
        nm.createNotificationChannel(NotificationChannel(CHANNEL, "USB cable", NotificationManager.IMPORTANCE_LOW))
        val stop = PendingIntent.getService(this, 3, Intent(this, UsbLinkService::class.java).setAction(ACTION_STOP), PendingIntent.FLAG_IMMUTABLE)
        val open = PendingIntent.getActivity(this, 4, Intent(this, MainActivity::class.java)
            .putExtra(MainActivity.EXTRA_TAB, 2).addFlags(Intent.FLAG_ACTIVITY_SINGLE_TOP), PendingIntent.FLAG_IMMUTABLE)
        return Notification.Builder(this, CHANNEL)
            .setSmallIcon(android.R.drawable.stat_sys_data_bluetooth)
            .setContentTitle("PC over USB cable")
            .setContentText(text)
            .setContentIntent(open)
            .setOngoing(true)
            .addAction(Notification.Action.Builder(null, "Disconnect", stop).build())
            .build()
    }

    override fun onDestroy() {
        state.value = null
        link?.stop(); link = null
        runCatching { unregisterReceiver(detached) }
        runCatching { pfd?.close() }; pfd = null
        super.onDestroy()
    }

    companion object {
        private const val CHANNEL = "usb"
        private const val ACTION_STOP = "stop"
        const val MANUFACTURER = "Hafij"
        const val MODEL = "Harbor Files"

        /** The PC on the cable, or null. */
        val state = mutableStateOf<UsbPc?>(null)

        fun ourAccessory(um: UsbManager): UsbAccessory? =
            um.accessoryList?.firstOrNull { it.manufacturer == MANUFACTURER && it.model == MODEL }

        /** Start the link if Harbor's PC accessory is plugged in and allowed. */
        fun ensure(ctx: Context) {
            if (state.value != null) return
            val um = ctx.getSystemService(UsbManager::class.java) ?: return
            val acc = ourAccessory(um) ?: return
            if (!um.hasPermission(acc)) return
            runCatching { ctx.startForegroundService(Intent(ctx, UsbLinkService::class.java)) }
        }

        fun stop(ctx: Context) {
            runCatching { ctx.startService(Intent(ctx, UsbLinkService::class.java).setAction(ACTION_STOP)) }
        }
    }
}

/** Opened by Android when the PC switches the phone to accessory mode. */
class UsbAttachActivity : Activity() {
    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        runCatching { startForegroundService(Intent(this, UsbLinkService::class.java)) }
        startActivity(Intent(this, MainActivity::class.java)
            .putExtra(MainActivity.EXTRA_TAB, 2)
            .addFlags(Intent.FLAG_ACTIVITY_NEW_TASK or Intent.FLAG_ACTIVITY_SINGLE_TOP))
        finish()
    }
}
