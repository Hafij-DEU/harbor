package com.hafij.harborfiles

import android.app.Notification
import android.app.NotificationChannel
import android.app.NotificationManager
import android.app.PendingIntent
import android.app.Service
import android.content.Context
import android.content.Intent
import android.content.pm.ServiceInfo
import android.net.ConnectivityManager
import android.net.NetworkCapabilities
import android.net.wifi.WifiManager
import android.os.Handler
import android.os.IBinder
import android.os.Looper
import android.os.PowerManager
import androidx.compose.runtime.mutableStateOf
import java.io.File
import java.net.Inet4Address
import java.security.SecureRandom

data class ServerInfo(val ip: String, val port: Int, val pin: String) {
    val url: String get() = "ftp://$ip:$port"
}

/** Runs [FtpServer] in the foreground while the phone is shared. Wi-Fi only; stops when idle. */
class ServerService : Service() {
    private var ftp: FtpServer? = null
    @Volatile private var lastActivity = 0L
    private val handler = Handler(Looper.getMainLooper())
    private var wake: PowerManager.WakeLock? = null
    private var wifiLock: WifiManager.WifiLock? = null
    private lateinit var prefs: Prefs

    override fun onBind(intent: Intent?): IBinder? = null

    override fun onStartCommand(intent: Intent?, flags: Int, startId: Int): Int {
        if (intent?.action == ACTION_STOP) { shutdown(); return START_NOT_STICKY }
        if (ftp != null) return START_NOT_STICKY
        prefs = Prefs(this)
        val ip = wifiIp(this)
        if (ip == null) {
            error.value = "Connect to Wi-Fi first. The phone is only shared on Wi-Fi."
            stopSelf(); return START_NOT_STICKY
        }
        val pin = (100000 + SecureRandom().nextInt(900000)).toString()
        val s = FtpServer(File(PHONE_ROOT), pin, prefs.ftpPort, { prefs.readOnly }, { lastActivity = System.currentTimeMillis() })
        try {
            s.start()
        } catch (e: Exception) {
            error.value = "Port ${prefs.ftpPort} is busy. Pick another port in Settings."
            stopSelf(); return START_NOT_STICKY
        }
        ftp = s
        lastActivity = System.currentTimeMillis()
        val info = ServerInfo(ip, s.boundPort, pin)
        state.value = info
        error.value = null
        startForeground(1, notification(info), ServiceInfo.FOREGROUND_SERVICE_TYPE_DATA_SYNC)
        wake = (getSystemService(POWER_SERVICE) as PowerManager)
            .newWakeLock(PowerManager.PARTIAL_WAKE_LOCK, "harbor:ftp").apply { acquire(6 * 3600_000L) }
        @Suppress("DEPRECATION")
        wifiLock = (applicationContext.getSystemService(WIFI_SERVICE) as WifiManager)
            .createWifiLock(WifiManager.WIFI_MODE_FULL_HIGH_PERF, "harbor:ftp").apply { acquire() }
        handler.postDelayed(watch, 30_000)
        return START_NOT_STICKY
    }

    private val watch = object : Runnable {
        override fun run() {
            val s = ftp ?: return
            val idle = prefs.idleMin
            val tooIdle = idle > 0 && s.activeSessions == 0 &&
                System.currentTimeMillis() - lastActivity > idle * 60_000L
            if (tooIdle || wifiIp(this@ServerService) != state.value?.ip) { shutdown(); return }
            handler.postDelayed(this, 30_000)
        }
    }

    private fun notification(i: ServerInfo): Notification {
        val nm = getSystemService(NotificationManager::class.java)
        nm.createNotificationChannel(NotificationChannel(CHANNEL, "Phone sharing", NotificationManager.IMPORTANCE_LOW))
        val stop = PendingIntent.getService(
            this, 0, Intent(this, ServerService::class.java).setAction(ACTION_STOP), PendingIntent.FLAG_IMMUTABLE
        )
        val open = PendingIntent.getActivity(
            this, 0, Intent(this, MainActivity::class.java).addFlags(Intent.FLAG_ACTIVITY_SINGLE_TOP), PendingIntent.FLAG_IMMUTABLE
        )
        return Notification.Builder(this, CHANNEL)
            .setSmallIcon(android.R.drawable.stat_sys_upload)
            .setContentTitle("Phone shared over Wi-Fi")
            .setContentText("${i.url}  ·  PIN in Harbor Files")
            .setContentIntent(open)
            .setOngoing(true)
            .addAction(Notification.Action.Builder(null, "Stop", stop).build())
            .build()
    }

    private fun shutdown() {
        handler.removeCallbacks(watch)
        ftp?.stop(); ftp = null
        runCatching { wake?.release() }; wake = null
        runCatching { wifiLock?.release() }; wifiLock = null
        state.value = null
        stopForeground(STOP_FOREGROUND_REMOVE)
        stopSelf()
    }

    override fun onDestroy() { if (ftp != null) shutdown(); super.onDestroy() }

    companion object {
        private const val CHANNEL = "share"
        const val ACTION_STOP = "stop"
        val state = mutableStateOf<ServerInfo?>(null)
        val error = mutableStateOf<String?>(null)

        fun start(ctx: Context) = ctx.startForegroundService(Intent(ctx, ServerService::class.java))
        fun stop(ctx: Context) = ctx.startService(Intent(ctx, ServerService::class.java).setAction(ACTION_STOP))

        fun wifiIp(ctx: Context): String? {
            val cm = ctx.getSystemService(ConnectivityManager::class.java)
            @Suppress("DEPRECATION")
            for (n in cm.allNetworks) {
                val caps = cm.getNetworkCapabilities(n) ?: continue
                if (!caps.hasTransport(NetworkCapabilities.TRANSPORT_WIFI)) continue
                val lp = cm.getLinkProperties(n) ?: continue
                lp.linkAddresses.map { it.address }.firstOrNull { it is Inet4Address && !it.isLoopbackAddress }
                    ?.let { return it.hostAddress }
            }
            return null
        }
    }
}
