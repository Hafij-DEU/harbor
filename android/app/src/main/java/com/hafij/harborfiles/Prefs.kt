package com.hafij.harborfiles

import android.content.Context
import android.security.keystore.KeyGenParameterSpec
import android.security.keystore.KeyProperties
import android.util.Base64
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import java.security.KeyStore
import javax.crypto.Cipher
import javax.crypto.KeyGenerator
import javax.crypto.SecretKey
import javax.crypto.spec.GCMParameterSpec

/** App settings, observable from Compose. */
class Prefs(ctx: Context) {
    private val sp = ctx.getSharedPreferences("harbor", Context.MODE_PRIVATE)

    private fun bool(k: String, d: Boolean) = mutableStateOf(sp.getBoolean(k, d))
    private fun int(k: String, d: Int) = mutableStateOf(sp.getInt(k, d))

    private val _showHidden = bool("showHidden", false)
    var showHidden: Boolean
        get() = _showHidden.value
        set(v) { _showHidden.value = v; sp.edit().putBoolean("showHidden", v).apply() }

    private val _hideSystem = bool("hideSystem", true)
    var hideSystem: Boolean
        get() = _hideSystem.value
        set(v) { _hideSystem.value = v; sp.edit().putBoolean("hideSystem", v).apply() }

    private val _hideScreenshots = bool("hideScreenshots", false)
    var hideScreenshots: Boolean
        get() = _hideScreenshots.value
        set(v) { _hideScreenshots.value = v; sp.edit().putBoolean("hideScreenshots", v).apply() }

    private val _theme = int("theme", 0) // 0 system, 1 light, 2 dark
    var theme: Int
        get() = _theme.value
        set(v) { _theme.value = v; sp.edit().putInt("theme", v).apply() }

    private val _accent = int("accent", 0)
    var accent: Int
        get() = _accent.value
        set(v) { _accent.value = v; sp.edit().putInt("accent", v).apply() }

    private val _trashDays = int("trashDays", 30) // 0 = delete at once
    var trashDays: Int
        get() = _trashDays.value
        set(v) { _trashDays.value = v; sp.edit().putInt("trashDays", v).apply() }

    private val _ftpPort = int("ftpPort", 2121)
    var ftpPort: Int
        get() = _ftpPort.value
        set(v) { _ftpPort.value = v; sp.edit().putInt("ftpPort", v).apply() }

    private val _readOnly = bool("readOnly", false)
    var readOnly: Boolean
        get() = _readOnly.value
        set(v) { _readOnly.value = v; sp.edit().putBoolean("readOnly", v).apply() }

    private val _idleMin = int("idleMin", 15) // 0 = never
    var idleMin: Int
        get() = _idleMin.value
        set(v) { _idleMin.value = v; sp.edit().putInt("idleMin", v).apply() }

    private val _meshdrop = bool("meshdrop", true)
    var meshdrop: Boolean
        get() = _meshdrop.value
        set(v) { _meshdrop.value = v; sp.edit().putBoolean("meshdrop", v).apply() }

    private val _sort = int("sort", 0) // 0 name, 1 date, 2 size, 3 type
    var sort: Int
        get() = _sort.value
        set(v) { _sort.value = v; sp.edit().putInt("sort", v).apply() }

    private val _sortDesc = bool("sortDesc", false)
    var sortDesc: Boolean
        get() = _sortDesc.value
        set(v) { _sortDesc.value = v; sp.edit().putBoolean("sortDesc", v).apply() }

    private val _favorites = mutableStateOf(sp.getStringSet("favorites", emptySet())!!.toSet())
    val favorites: Set<String> get() = _favorites.value
    fun toggleFavorite(path: String) {
        val s = if (path in favorites) favorites - path else favorites + path
        _favorites.value = s; sp.edit().putStringSet("favorites", s).apply()
    }

    private val _servers = mutableStateOf(
        sp.getString("servers", "")!!.lines().mapNotNull { Server.decode(it) }
    )
    val servers: List<Server> get() = _servers.value
    fun saveServer(s: Server) {
        val l = servers.filter { it.id != s.id } + s
        _servers.value = l; sp.edit().putString("servers", l.joinToString("\n") { it.encode() }).apply()
    }
    fun removeServer(id: String) {
        val l = servers.filter { it.id != id }
        _servers.value = l; sp.edit().putString("servers", l.joinToString("\n") { it.encode() }).apply()
        sp.edit().remove("secret_$id").apply()
    }

    fun setSecret(id: String, value: String) { sp.edit().putString("secret_$id", Secret.seal(value)).apply() }
    fun secret(id: String): String = sp.getString("secret_$id", null)?.let { Secret.open(it) } ?: ""
}

/** AES-GCM with a key held in the Android Keystore; the PIN never sits in plain text. */
object Secret {
    private const val ALIAS = "harbor-servers"

    private fun key(): SecretKey {
        val ks = KeyStore.getInstance("AndroidKeyStore").apply { load(null) }
        (ks.getKey(ALIAS, null) as? SecretKey)?.let { return it }
        val g = KeyGenerator.getInstance(KeyProperties.KEY_ALGORITHM_AES, "AndroidKeyStore")
        g.init(
            KeyGenParameterSpec.Builder(ALIAS, KeyProperties.PURPOSE_ENCRYPT or KeyProperties.PURPOSE_DECRYPT)
                .setBlockModes(KeyProperties.BLOCK_MODE_GCM)
                .setEncryptionPaddings(KeyProperties.ENCRYPTION_PADDING_NONE)
                .build()
        )
        return g.generateKey()
    }

    fun seal(plain: String): String {
        val c = Cipher.getInstance("AES/GCM/NoPadding").apply { init(Cipher.ENCRYPT_MODE, key()) }
        val ct = c.doFinal(plain.toByteArray())
        return Base64.encodeToString(c.iv, Base64.NO_WRAP) + ":" + Base64.encodeToString(ct, Base64.NO_WRAP)
    }

    fun open(sealed: String): String = runCatching {
        val (iv, ct) = sealed.split(':').map { Base64.decode(it, Base64.NO_WRAP) }
        val c = Cipher.getInstance("AES/GCM/NoPadding").apply { init(Cipher.DECRYPT_MODE, key(), GCMParameterSpec(128, iv)) }
        String(c.doFinal(ct))
    }.getOrDefault("")
}
