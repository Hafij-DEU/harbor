package com.hafij.harborfiles

import java.io.File
import java.io.InputStream
import java.text.DateFormat
import java.util.Date
import java.util.Locale

/** One file or folder, local or on a server. */
data class Entry(
    val name: String,
    val path: String,
    val isDir: Boolean,
    val size: Long = 0,
    val modified: Long = 0,
    val items: Int = -1,
    val mime: String? = null,
) {
    val kind: Kind get() = if (isDir) Kind.Folder else kindOf(name, mime)
}

enum class Kind { Folder, Image, Video, Audio, Doc, Archive, Apk, Other }

private val DOC_EXT = setOf(
    "pdf", "doc", "docx", "odt", "rtf", "txt", "md", "xls", "xlsx", "ods", "csv", "ppt", "pptx",
    "odp", "epub", "html", "htm", "json", "xml", "log",
)
private val ARCHIVE_EXT = setOf("zip", "rar", "7z", "tar", "gz", "tgz", "bz2", "xz", "zst")
private val IMAGE_EXT = setOf("jpg", "jpeg", "png", "gif", "webp", "heic", "heif", "bmp", "jxl", "avif", "svg")
private val VIDEO_EXT = setOf("mp4", "mkv", "webm", "mov", "avi", "3gp", "m4v", "ts")
private val AUDIO_EXT = setOf("mp3", "m4a", "aac", "ogg", "opus", "flac", "wav", "amr", "mid")

fun ext(name: String) = name.substringAfterLast('.', "").lowercase(Locale.ROOT)

fun kindOf(name: String, mime: String? = null): Kind {
    val e = ext(name)
    return when {
        e == "apk" || e == "apks" || e == "xapk" -> Kind.Apk
        mime?.startsWith("image/") == true || e in IMAGE_EXT -> Kind.Image
        mime?.startsWith("video/") == true || e in VIDEO_EXT -> Kind.Video
        mime?.startsWith("audio/") == true || e in AUDIO_EXT -> Kind.Audio
        e in ARCHIVE_EXT -> Kind.Archive
        e in DOC_EXT || mime?.startsWith("text/") == true -> Kind.Doc
        else -> Kind.Other
    }
}

fun mimeOf(name: String): String =
    android.webkit.MimeTypeMap.getSingleton().getMimeTypeFromExtension(ext(name)) ?: "*/*"

fun fmtSize(n: Long): String {
    if (n < 1024) return "$n B"
    val u = arrayOf("KB", "MB", "GB", "TB")
    var v = n / 1024.0
    var i = 0
    while (v >= 1024 && i < u.size - 1) { v /= 1024; i++ }
    return if (v >= 100) String.format(Locale.US, "%.0f %s", v, u[i])
    else String.format(Locale.US, "%.1f %s", v, u[i])
}

fun fmtDate(ms: Long): String =
    if (ms <= 0) "" else DateFormat.getDateTimeInstance(DateFormat.SHORT, DateFormat.SHORT).format(Date(ms))

fun subtitle(e: Entry): String {
    val d = fmtDate(e.modified)
    val r = if (e.isDir) (if (e.items >= 0) "${e.items} items" else "") else fmtSize(e.size)
    return listOf(d, r).filter { it.isNotEmpty() }.joinToString("  |  ")
}

/** A place files live: this phone, or a server. All calls block; run them off the main thread. */
interface Fs {
    val key: String
    val title: String
    val root: String
    val isLocal: Boolean get() = false
    fun list(path: String): List<Entry>
    fun mkdir(parent: String, name: String)
    fun delete(e: Entry)
    fun rename(e: Entry, newName: String)
    fun <T> read(path: String, block: (InputStream) -> T): T
    fun write(dir: String, name: String, size: Long, input: InputStream)
    fun join(dir: String, name: String) = if (dir.endsWith("/")) dir + name else "$dir/$name"
    fun nameOf(path: String) = path.trimEnd('/').substringAfterLast('/')
}

/** "a.txt" -> "a (1).txt" until the name is free. */
fun uniqueName(name: String, taken: (String) -> Boolean): String {
    if (!taken(name)) return name
    val dot = name.lastIndexOf('.')
    val (base, ex) = if (dot > 0) name.substring(0, dot) to name.substring(dot) else name to ""
    var i = 1
    while (true) {
        val n = "$base ($i)$ex"
        if (!taken(n)) return n
        i++
    }
}

object Transfer {
    /** Copies [e] from [src] into [dstDir] on [dst], folders included. Returns the name used. */
    fun copy(src: Fs, e: Entry, dst: Fs, dstDir: String, progress: (String) -> Unit): String {
        val name = if (dst.isLocal) uniqueName(e.name) { File(dstDir, it).exists() } else e.name
        progress(e.name)
        if (e.isDir) {
            try { dst.mkdir(dstDir, name) } catch (_: Exception) { /* exists */ }
            val d = dst.join(dstDir, name)
            for (c in src.list(e.path)) copy(src, c, dst, d, progress)
        } else {
            src.read(e.path) { dst.write(dstDir, name, e.size, it) }
        }
        return name
    }

    /** Fast rename on the same phone volume, else copy then delete. */
    fun move(src: Fs, e: Entry, dst: Fs, dstDir: String, progress: (String) -> Unit) {
        if (src.isLocal && dst.isLocal) {
            val from = File(e.path)
            val to = File(dstDir, uniqueName(e.name) { File(dstDir, it).exists() })
            if (from.parentFile?.canonicalPath == File(dstDir).canonicalPath) return
            progress(e.name)
            if (from.renameTo(to)) return
        }
        copy(src, e, dst, dstDir, progress)
        src.delete(e)
    }
}
