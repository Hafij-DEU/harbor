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
import android.provider.Settings
import androidx.compose.runtime.mutableStateOf
import java.io.File
import java.net.Inet4Address
import java.security.SecureRandom

/** [via]: how the PC reaches the phone ("Wi-Fi", "hotspot", "USB tethering"...). */
data class ServerInfo(val ip: String, val port: Int, val pin: String, val via: String = "Wi-Fi") {
    val url: String get() = "ftp://$ip:$port"
    /** The QR code carries the login too, so a scan connects with no typing. */
    val qrText: String get() = "ftp://harbor:$pin@$ip:$port/"
}

/**
 * Runs [FtpServer] in the foreground while the phone is shared: on Wi-Fi, on
 * the phone's own hotspot or over USB tethering (never mobile data). Stops when idle.
 */
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
        val (ip, via) = shareAddrs(this).firstOrNull() ?: run {
            fail("Connect to Wi-Fi, or turn on the hotspot or USB tethering first. The phone is never shared on mobile data.")
            return START_NOT_STICKY
        }
        val pin = (100000 + SecureRandom().nextInt(900000)).toString()
        val s = FtpServer(File(PHONE_ROOT), pin, prefs.ftpPort, { prefs.readOnly }, { lastActivity = System.currentTimeMillis() },
            deviceName())
        try {
            s.start()
        } catch (e: Exception) {
            fail("Port ${prefs.ftpPort} is busy. Pick another port in Settings."); return START_NOT_STICKY
        }
        ftp = s
        lastActivity = System.currentTimeMillis()
        val info = ServerInfo(ip, s.boundPort, pin, via)
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
            if (tooIdle || shareAddrs(this@ServerService).none { it.first == state.value?.ip }) { shutdown(); return }
            handler.postDelayed(this, 30_000)
        }
    }

    /**
     * Started with startForegroundService, so Android needs startForeground
     * even when sharing can't start, or it kills the app. Shows the error,
     * then drops the notification and stops at once.
     */
    private fun fail(msg: String) {
        error.value = msg
        runCatching {
            ensureChannel()
            val n = Notification.Builder(this, CHANNEL)
                .setSmallIcon(android.R.drawable.stat_sys_upload)
                .setContentTitle("Phone sharing didn't start")
                .setContentText(msg)
                .build()
            startForeground(1, n, ServiceInfo.FOREGROUND_SERVICE_TYPE_DATA_SYNC)
        }
        runCatching { stopForeground(STOP_FOREGROUND_REMOVE) }
        stopSelf()
    }

    private fun ensureChannel() {
        val nm = getSystemService(NotificationManager::class.java)
        nm.createNotificationChannel(NotificationChannel(CHANNEL, "Phone sharing", NotificationManager.IMPORTANCE_LOW))
    }

    private fun notification(i: ServerInfo): Notification {
        ensureChannel()
        val stop = PendingIntent.getService(
            this, 0, Intent(this, ServerService::class.java).setAction(ACTION_STOP), PendingIntent.FLAG_IMMUTABLE
        )
        val open = PendingIntent.getActivity(
            this, 0, Intent(this, MainActivity::class.java).addFlags(Intent.FLAG_ACTIVITY_SINGLE_TOP), PendingIntent.FLAG_IMMUTABLE
        )
        val qr = PendingIntent.getActivity(
            this, 1, Intent(this, MainActivity::class.java).addFlags(Intent.FLAG_ACTIVITY_SINGLE_TOP)
                .putExtra(MainActivity.EXTRA_TAB, 2), PendingIntent.FLAG_IMMUTABLE or PendingIntent.FLAG_UPDATE_CURRENT
        )
        return Notification.Builder(this, CHANNEL)
            .setSmallIcon(android.R.drawable.stat_sys_upload)
            .setContentTitle("Phone shared over Wi-Fi")
            .setContentText("${i.url}  ·  PIN in Harbor Files")
            .setContentIntent(open)
            .setOngoing(true)
            .addAction(Notification.Action.Builder(null, "Show QR", qr).build())
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

        /**
         * The phone's address on the Wi-Fi network, the one the PC can reach.
         * A VPN (e.g. Proton VPN) also reports Wi-Fi as its transport, and its
         * tunnel address (10.2.0.2 on tun0) is useless to the PC, so VPN
         * networks and tunnel interfaces are skipped.
         */
        /**
         * Addresses the PC can use, best first: Wi-Fi, then the phone's own
         * hotspot, USB tethering, Ethernet and Bluetooth tethering. Pairs of
         * (IPv4, how). Mobile data and VPN tunnels never count.
         */
        fun shareAddrs(ctx: Context): List<Pair<String, String>> {
            val out = ArrayList<Pair<String, String>>()
            wifiIp(ctx)?.let { out += it to "Wi-Fi" }
            val ifs = runCatching { java.net.NetworkInterface.getNetworkInterfaces()?.toList() }.getOrNull().orEmpty()
                .filter { runCatching { it.isUp && !it.isLoopback }.getOrDefault(false) }
            out += chooseTetherIps(ifs.map { IpCandidate(it.name, false, it.inetAddresses.toList()) })
            return out.distinctBy { it.first }
        }

        fun wifiIp(ctx: Context): String? {
            val cm = ctx.getSystemService(ConnectivityManager::class.java)
            val cands = ArrayList<IpCandidate>()
            @Suppress("DEPRECATION")
            for (n in cm.allNetworks) {
                val caps = cm.getNetworkCapabilities(n) ?: continue
                if (!caps.hasTransport(NetworkCapabilities.TRANSPORT_WIFI)) continue
                val lp = cm.getLinkProperties(n) ?: continue
                cands += IpCandidate(lp.interfaceName ?: "", caps.hasTransport(NetworkCapabilities.TRANSPORT_VPN),
                    lp.linkAddresses.map { it.address })
            }
            chooseWifiIp(cands)?.let { return it }
            // Fallback: the Wi-Fi interface itself (wlan0 and friends).
            val ifs = runCatching { java.net.NetworkInterface.getNetworkInterfaces()?.toList() }.getOrNull().orEmpty()
            return chooseWifiIp(ifs.filter { runCatching { it.isUp }.getOrDefault(false) && it.name.startsWith("wlan") }
                .map { IpCandidate(it.name, false, it.inetAddresses.toList()) })
        }
    }

    private fun deviceName(): String =
        (try { Settings.Global.getString(contentResolver, "device_name") } catch (_: Exception) { null })
            ?.takeIf { it.isNotBlank() } ?: android.os.Build.MODEL ?: ""
}

/** One network's addresses, for [chooseWifiIp]. */
internal data class IpCandidate(val iface: String, val vpn: Boolean, val addrs: List<java.net.InetAddress>)

private val TUNNEL_IFACES = listOf("tun", "ppp", "ipsec", "wg", "tap", "rmnet", "ccmni")

/** First usable IPv4 of a real Wi-Fi network: no VPN, tunnel, loopback or link-local address. */
internal fun chooseWifiIp(cands: List<IpCandidate>): String? {
    for (c in cands) {
        if (c.vpn || TUNNEL_IFACES.any { c.iface.startsWith(it) }) continue
        c.addrs.firstOrNull { it is Inet4Address && !it.isLoopbackAddress && !it.isLinkLocalAddress && !it.isAnyLocalAddress }
            ?.let { return it.hostAddress }
    }
    return null
}

/** How the PC reaches the phone through an interface, by its name, or null (not a local link). */
internal fun tetherKind(iface: String): String? = when {
    TUNNEL_IFACES.any { iface.startsWith(it) } -> null
    listOf("rndis", "usb", "ncm").any { iface.startsWith(it) } -> "USB tethering"
    listOf("ap", "swlan", "softap", "wlan").any { iface.startsWith(it) } -> "hotspot"
    iface.startsWith("eth") -> "Ethernet"
    iface.startsWith("bt-pan") || iface.startsWith("bnep") -> "Bluetooth tethering"
    else -> null
}

/**
 * Private IPv4 addresses on the phone's hotspot / tethering interfaces, best
 * first (USB, hotspot, Ethernet, Bluetooth). Only private (site-local)
 * addresses: never a public or mobile-data one.
 */
internal fun chooseTetherIps(cands: List<IpCandidate>): List<Pair<String, String>> {
    val order = listOf("USB tethering", "hotspot", "Ethernet", "Bluetooth tethering")
    return cands.mapNotNull { c ->
        val kind = if (c.vpn) null else tetherKind(c.iface)
        val ip = c.addrs.firstOrNull { it is Inet4Address && it.isSiteLocalAddress }
        if (kind != null && ip != null) ip.hostAddress!! to kind else null
    }.sortedBy { order.indexOf(it.second) }
}
