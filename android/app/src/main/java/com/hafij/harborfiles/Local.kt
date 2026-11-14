package com.hafij.harborfiles

import android.content.ContentResolver
import android.content.Context
import android.database.Cursor
import android.media.MediaScannerConnection
import android.net.Uri
import android.os.Build
import android.os.Bundle
import android.os.Environment
import android.provider.MediaStore
import java.io.File
import java.io.FileOutputStream
import java.io.IOException
import java.io.InputStream
import java.util.zip.ZipEntry
import java.util.zip.ZipInputStream
import java.util.zip.ZipOutputStream

val PHONE_ROOT: String get() = Environment.getExternalStorageDirectory().path
const val TRASH_DIR = ".HarborTrash"

/** Files on this phone. Paths are absolute. */
class LocalFs(
    private val showHidden: () -> Boolean = { false },
    private val hideSystem: () -> Boolean = { true },
) : Fs {
    override val key = "local"
    override val title = "Internal storage"
    override val root: String get() = PHONE_ROOT
    override val isLocal = true

    private fun visible(f: File, atRoot: Boolean): Boolean {
        val n = f.name
        if (n == TRASH_DIR) return false
        if (!showHidden() && n.startsWith(".")) return false
        if (atRoot && hideSystem() && (n == "Android" || n == "MIUI" || n == "LOST.DIR")) return false
        return true
    }

    override fun list(path: String): List<Entry> {
        val dir = File(path)
        val atRoot = path.trimEnd('/') == root
        val files = dir.listFiles() ?: throw IOException("Can't open ${dir.name}")
        return files.filter { visible(it, atRoot) }.map { f ->
            val d = f.isDirectory
            Entry(
                name = f.name, path = f.path, isDir = d,
                size = if (d) 0 else f.length(), modified = f.lastModified(),
                items = if (d) (f.list()?.count { showHidden() || !it.startsWith(".") } ?: 0) else -1,
            )
        }
    }

    override fun mkdir(parent: String, name: String) {
        val f = File(parent, name)
        if (f.exists()) throw IOException("$name already exists")
        if (!f.mkdirs()) throw IOException("Can't create $name")
    }

    override fun delete(e: Entry) {
        if (!File(e.path).deleteRecursively()) throw IOException("Can't delete ${e.name}")
    }

    override fun rename(e: Entry, newName: String) {
        val to = File(File(e.path).parentFile, newName)
        if (to.exists()) throw IOException("$newName already exists")
        if (!File(e.path).renameTo(to)) throw IOException("Can't rename ${e.name}")
    }

    override fun <T> read(path: String, block: (InputStream) -> T): T = File(path).inputStream().use(block)

    override fun write(dir: String, name: String, size: Long, input: InputStream) {
        File(dir).mkdirs()
        val part = File(dir, ".$name.part")
        part.outputStream().use { input.copyTo(it, 256 * 1024) }
        val dest = File(dir, name)
        if (dest.exists()) dest.delete()
        if (!part.renameTo(dest)) { part.delete(); throw IOException("Can't save $name") }
    }
}

/** 30-day bin in <volume>/.HarborTrash. Index lines: id \t deletedAt \t isDir \t size \t origPath */
class Trash(val dir: File) {
    data class Item(val id: String, val deletedAt: Long, val isDir: Boolean, val size: Long, val origPath: String) {
        val name: String get() = origPath.substringAfterLast('/')
    }

    private val index get() = File(dir, ".index")

    @Synchronized
    fun list(): List<Item> {
        if (!index.exists()) return emptyList()
        return index.readLines().mapNotNull { l ->
            val p = l.split('\t')
            if (p.size < 5) null
            else Item(p[0], p[1].toLongOrNull() ?: 0, p[2] == "1", p[3].toLongOrNull() ?: 0, p.drop(4).joinToString("\t"))
        }.filter { File(dir, it.id).exists() }.sortedByDescending { it.deletedAt }
    }

    private fun save(items: List<Item>) {
        dir.mkdirs()
        index.writeText(items.joinToString("") {
            "${it.id}\t${it.deletedAt}\t${if (it.isDir) 1 else 0}\t${it.size}\t${it.origPath}\n"
        })
    }

    @Synchronized
    fun put(f: File, now: Long = System.currentTimeMillis()): Item {
        dir.mkdirs()
        val id = "${now}_${(1000..9999).random()}"
        val size = if (f.isDirectory) f.walkBottomUp().filter { it.isFile }.sumOf { it.length() } else f.length()
        val dest = File(dir, id)
        if (!f.renameTo(dest)) {
            f.copyRecursively(dest, overwrite = false)
            f.deleteRecursively()
        }
        val item = Item(id, now, dest.isDirectory, size, f.path)
        save(list() + item)
        return item
    }

    @Synchronized
    fun restore(item: Item): File {
        val orig = File(item.origPath)
        orig.parentFile?.mkdirs()
        val to = File(orig.parentFile, uniqueName(orig.name) { File(orig.parentFile, it).exists() })
        val src = File(dir, item.id)
        if (!src.renameTo(to)) {
            src.copyRecursively(to)
            src.deleteRecursively()
        }
        save(list().filter { it.id != item.id })
        return to
    }

    @Synchronized
    fun purge(item: Item) {
        File(dir, item.id).deleteRecursively()
        save(list().filter { it.id != item.id })
    }

    /** Deletes items older than [days]; returns how many went. */
    @Synchronized
    fun expire(days: Int, now: Long = System.currentTimeMillis()): Int {
        if (days <= 0) return 0
        val cut = now - days * 86_400_000L
        val old = list().filter { it.deletedAt < cut }
        old.forEach { File(dir, it.id).deleteRecursively() }
        if (old.isNotEmpty()) save(list().filter { it.deletedAt >= cut })
        return old.size
    }

    fun daysLeft(item: Item, days: Int, now: Long = System.currentTimeMillis()): Int =
        ((item.deletedAt + days * 86_400_000L - now) / 86_400_000L).toInt().coerceAtLeast(0) + 1
}

object Zip {
    fun compress(sources: List<File>, out: File) {
        ZipOutputStream(FileOutputStream(out).buffered()).use { zip ->
            for (s in sources) {
                val base = s.parentFile ?: continue
                s.walkTopDown().forEach { f ->
                    val rel = f.relativeTo(base).path + if (f.isDirectory) "/" else ""
                    zip.putNextEntry(ZipEntry(rel).apply { time = f.lastModified() })
                    if (f.isFile) f.inputStream().use { it.copyTo(zip) }
                    zip.closeEntry()
                }
            }
        }
    }

    /** Extracts into [dest]; refuses entries that climb out of it. */
    fun extract(zipFile: File, dest: File) {
        val root = dest.canonicalFile
        ZipInputStream(zipFile.inputStream().buffered()).use { zip ->
            while (true) {
                val e = zip.nextEntry ?: break
                val f = File(root, e.name).canonicalFile
                if (!f.path.startsWith(root.path + File.separator) && f != root) throw IOException("Bad entry ${e.name}")
                if (e.isDirectory) f.mkdirs() else {
                    f.parentFile?.mkdirs()
                    f.outputStream().use { zip.copyTo(it) }
                    if (e.time > 0) f.setLastModified(e.time)
                }
            }
        }
    }
}

/** Tells MediaStore about changed paths so Recent and categories stay right. */
fun rescan(ctx: Context, paths: Collection<String>) {
    if (paths.isEmpty()) return
    MediaScannerConnection.scanFile(ctx.applicationContext, paths.toTypedArray(), null, null)
}

enum class Cat(val label: String) { Docs("Docs"), Images("Images"), Videos("Videos"), Music("Music"), Archives("Archives"), Apks("APKs") }

object Media {
    private val FILES: Uri = MediaStore.Files.getContentUri("external")
    private val PROJ = arrayOf(
        MediaStore.Files.FileColumns._ID, MediaStore.Files.FileColumns.DATA,
        MediaStore.Files.FileColumns.DISPLAY_NAME, MediaStore.Files.FileColumns.MIME_TYPE,
        MediaStore.Files.FileColumns.SIZE, MediaStore.Files.FileColumns.DATE_MODIFIED,
        MediaStore.Files.FileColumns.DATE_ADDED,
    )
    private const val NOT_HIDDEN = "_data NOT LIKE '%/.%' AND _data NOT LIKE '%/Android/data/%'"

    private val DOC_MIME = listOf(
        "application/pdf", "application/msword", "application/vnd.ms-excel", "application/vnd.ms-powerpoint",
        "application/rtf", "application/epub+zip", "application/json",
    )

    fun catSelection(c: Cat): String = when (c) {
        Cat.Images -> "media_type=1"
        Cat.Videos -> "media_type=3"
        Cat.Music -> "media_type=2"
        Cat.Apks -> "(_data LIKE '%.apk' OR _data LIKE '%.apks' OR _data LIKE '%.xapk')"
        Cat.Archives -> listOf("zip", "rar", "7z", "tar", "gz", "tgz", "bz2", "xz").joinToString(" OR ", "(", ")") { "_data LIKE '%.$it'" }
        Cat.Docs -> "(mime_type LIKE 'text/%' OR mime_type LIKE 'application/vnd.openxmlformats%' OR " +
            "mime_type LIKE 'application/vnd.oasis%' OR mime_type IN (" + DOC_MIME.joinToString(",") { "'$it'" } + "))"
    }

    private fun query(cr: ContentResolver, sel: String, args: Array<String>?, sort: String, limit: Int): Cursor? =
        if (Build.VERSION.SDK_INT >= 30) {
            cr.query(FILES, PROJ, Bundle().apply {
                putString(ContentResolver.QUERY_ARG_SQL_SELECTION, sel)
                if (args != null) putStringArray(ContentResolver.QUERY_ARG_SQL_SELECTION_ARGS, args)
                putString(ContentResolver.QUERY_ARG_SQL_SORT_ORDER, sort)
                if (limit > 0) putInt(ContentResolver.QUERY_ARG_LIMIT, limit)
            }, null)
        } else {
            cr.query(FILES, PROJ, sel, args, if (limit > 0) "$sort LIMIT $limit" else sort)
        }

    private fun read(c: Cursor?): List<Entry> {
        val out = ArrayList<Entry>()
        c?.use {
            while (it.moveToNext()) {
                val path = it.getString(1) ?: continue
                val name = it.getString(2) ?: path.substringAfterLast('/')
                val mime = it.getString(3)
                val mod = it.getLong(5).takeIf { m -> m > 0 } ?: it.getLong(6)
                val isDir = mime == null && File(path).isDirectory
                out += Entry(name, path, isDir, it.getLong(4), mod * 1000, mime = mime)
            }
        }
        return out
    }

    fun recent(ctx: Context, limit: Int = 500): List<Entry> =
        read(query(ctx.contentResolver, "mime_type IS NOT NULL AND $NOT_HIDDEN", null, "date_added DESC", limit))
            .filter { File(it.path).exists() }

    fun category(ctx: Context, c: Cat, limit: Int = 0): List<Entry> =
        read(query(ctx.contentResolver, "${catSelection(c)} AND $NOT_HIDDEN", null, "date_modified DESC", limit))

    fun count(ctx: Context, c: Cat): Int =
        query(ctx.contentResolver, "${catSelection(c)} AND $NOT_HIDDEN", null, "_id", 0)?.use { it.count } ?: 0

    fun search(ctx: Context, q: String, c: Cat?): List<Entry> {
        val like = "%" + q.replace("\\", "\\\\").replace("%", "\\%").replace("_", "\\_") + "%"
        val sel = "_display_name LIKE ? ESCAPE '\\' AND $NOT_HIDDEN" + (c?.let { " AND " + catSelection(it) } ?: "")
        return read(query(ctx.contentResolver, sel, arrayOf(like), "date_modified DESC", 300))
    }
}

/** Where a file came from, guessed from its folder. */
fun sourceOf(path: String): String {
    val p = path.removePrefix(PHONE_ROOT).lowercase()
    return when {
        "/dcim/camera" in p -> "Camera"
        "screenrecord" in p || "screen record" in p -> "Screen recordings"
        "screenshot" in p -> "Screenshots"
        "meshdrop" in p -> "MeshDrop"
        "whatsapp" in p -> "WhatsApp"
        "telegram" in p -> "Telegram"
        "bluetooth" in p -> "Bluetooth"
        "instagram" in p -> "Instagram"
        "sound_recorder" in p || "/recordings" in p || "/recorder" in p -> "Recorder"
        p.startsWith("/download") -> "Downloads"
        p.startsWith("/documents") -> "Documents"
        else -> path.substringBeforeLast('/').substringAfterLast('/').ifEmpty { "Phone" }
    }
}

/** Known app folders shown under Sources on Browse, if they exist. */
fun sourceDirs(): List<Pair<String, File>> {
    val r = File(PHONE_ROOT)
    val cand = listOf(
        "Camera" to "DCIM/Camera",
        "Screenshots" to "Pictures/Screenshots",
        "Screenshots" to "DCIM/Screenshots",
        "Screen recordings" to "Movies/ScreenRecorder",
        "Screen recordings" to "DCIM/ScreenRecorder",
        "MeshDrop" to "Download/MeshDrop",
        "MeshDrop" to "MeshDrop",
        "WhatsApp" to "Android/media/com.whatsapp/WhatsApp/Media",
        "Telegram" to "Download/Telegram",
        "Telegram" to "Telegram",
        "Bluetooth" to "Download/Bluetooth",
        "Bluetooth" to "Bluetooth",
        "Recorder" to "MIUI/sound_recorder",
        "Recorder" to "Recordings",
        "Instagram" to "Pictures/Instagram",
    )
    val seen = HashSet<String>()
    return cand.mapNotNull { (n, rel) ->
        val f = File(r, rel)
        if (n !in seen && f.isDirectory) { seen += n; n to f } else null
    }
}
