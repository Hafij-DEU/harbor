package com.hafij.harborfiles

import android.app.Notification
import android.app.NotificationChannel
import android.app.NotificationManager
import android.app.PendingIntent
import android.app.Service
import android.content.Context
import android.content.Intent
import android.content.pm.ServiceInfo
import android.net.wifi.WifiManager
import android.os.Handler
import android.os.IBinder
import android.os.Looper
import android.os.PowerManager

/**
 * Keeps Harbor awake while a player streams through [StreamProxy].
 *
 * The player is in front, so Harbor itself is a background app: phones
 * (TECNO/HiOS, MIUI...) then slow it down and let Wi-Fi doze, and the video
 * stalls. A foreground service with a Wi-Fi high-performance lock and a
 * partial wake lock keeps the relay quick. Stops by itself 2 minutes after
 * the player's last request, or from the notification.
 */
class StreamService : Service() {
    private var wake: PowerManager.WakeLock? = null
    private var wifiLock: WifiManager.WifiLock? = null
    private val handler = Handler(Looper.getMainLooper())
    private val check = object : Runnable {
        override fun run() {
            val p = StreamProxy.shared
            if (p.active.get() == 0 && System.currentTimeMillis() - p.lastUse > IDLE_MS) { stopSelf(); return }
            handler.postDelayed(this, 15_000)
        }
    }

    override fun onBind(intent: Intent?): IBinder? = null

    override fun onStartCommand(intent: Intent?, flags: Int, startId: Int): Int {
        if (intent?.action == ACTION_STOP) {
            StreamProxy.shared.close()
            stopSelf(); return START_NOT_STICKY
        }
        startForeground(2, notification(), ServiceInfo.FOREGROUND_SERVICE_TYPE_DATA_SYNC)
        if (wake == null) wake = getSystemService(PowerManager::class.java)
            .newWakeLock(PowerManager.PARTIAL_WAKE_LOCK, "harbor:stream").apply { acquire(4 * 3600_000L) }
        @Suppress("DEPRECATION")
        if (wifiLock == null) wifiLock = applicationContext.getSystemService(WifiManager::class.java)
            .createWifiLock(WifiManager.WIFI_MODE_FULL_HIGH_PERF, "harbor:stream").apply { acquire() }
        StreamProxy.shared.lastUse = System.currentTimeMillis()
        handler.removeCallbacks(check)
        handler.postDelayed(check, 15_000)
        return START_NOT_STICKY
    }

    private fun notification(): Notification {
        val nm = getSystemService(NotificationManager::class.java)
        nm.createNotificationChannel(NotificationChannel(CHANNEL, "Streaming", NotificationManager.IMPORTANCE_LOW))
        val stop = PendingIntent.getService(
            this, 1, Intent(this, StreamService::class.java).setAction(ACTION_STOP), PendingIntent.FLAG_IMMUTABLE
        )
        return Notification.Builder(this, CHANNEL)
            .setSmallIcon(android.R.drawable.stat_sys_download)
            .setContentTitle("Streaming from the PC")
            .setContentText("Harbor Files passes the video to your player")
            .setOngoing(true)
            .addAction(Notification.Action.Builder(null, "Stop", stop).build())
            .build()
    }

    override fun onDestroy() {
        handler.removeCallbacks(check)
        runCatching { wake?.release() }; wake = null
        runCatching { wifiLock?.release() }; wifiLock = null
        super.onDestroy()
    }

    companion object {
        private const val CHANNEL = "stream"
        private const val ACTION_STOP = "stop"
        private const val IDLE_MS = 2 * 60_000L

        fun start(ctx: Context) {
            runCatching { ctx.startForegroundService(Intent(ctx, StreamService::class.java)) }
        }
    }
}
