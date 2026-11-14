package com.hafij.harborfiles

import android.content.ActivityNotFoundException
import android.content.ClipData
import android.content.ClipboardManager
import android.content.Context
import android.content.Intent
import android.net.Uri
import androidx.core.content.FileProvider
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext
import java.io.File

const val MESHDROP_PKG = "io.github.hafijbge.meshdrop"

fun meshdropInstalled(ctx: Context) = runCatching { ctx.packageManager.getPackageInfo(MESHDROP_PKG, 0); true }.getOrDefault(false)

private fun uriFor(ctx: Context, f: File): Uri = FileProvider.getUriForFile(ctx, "com.hafij.harborfiles.files", f)

/** Local file for [e]: itself on the phone, or a copy in the cache for a server file. */
private fun localCopy(ctx: Context, fs: Fs, e: Entry): File {
    if (fs.isLocal) return File(e.path)
    val dir = File(ctx.cacheDir, "remote/${fs.key}").apply { mkdirs() }
    val f = File(dir, e.name)
    fs.read(e.path) { i -> f.outputStream().use { i.copyTo(it) } }
    return f
}

suspend fun openEntry(state: AppState, fs: Fs, e: Entry, chooser: Boolean = false) {
    val ctx = state.ctx
    if (!fs.isLocal) state.busy = "Downloading ${e.name}…"
    val f = try { withContext(Dispatchers.IO) { localCopy(ctx, fs, e) } } catch (x: Exception) {
        state.toast(x.message ?: "Couldn't open"); return
    } finally { state.busy = null }
    val mime = e.mime ?: mimeOf(e.name)
    val i = Intent(Intent.ACTION_VIEW).setDataAndType(uriFor(ctx, f), mime)
        .addFlags(Intent.FLAG_GRANT_READ_URI_PERMISSION or Intent.FLAG_GRANT_WRITE_URI_PERMISSION or Intent.FLAG_ACTIVITY_NEW_TASK)
    try {
        ctx.startActivity(if (chooser) Intent.createChooser(i, "Open with").addFlags(Intent.FLAG_ACTIVITY_NEW_TASK) else i)
    } catch (_: ActivityNotFoundException) {
        state.toast("No app can open ${e.name}")
    }
}

suspend fun shareEntries(state: AppState, fs: Fs, items: List<Entry>, meshdrop: Boolean) {
    val ctx = state.ctx
    val files = items.filter { !it.isDir }
    if (files.isEmpty()) { state.toast("Pick files, not folders, to send"); return }
    if (!fs.isLocal) state.busy = "Downloading…"
    val uris = try {
        withContext(Dispatchers.IO) { files.map { uriFor(ctx, localCopy(ctx, fs, it)) } }
    } catch (x: Exception) { state.toast(x.message ?: "Couldn't send"); return } finally { state.busy = null }
    val i = if (uris.size == 1) Intent(Intent.ACTION_SEND).putExtra(Intent.EXTRA_STREAM, uris[0])
    else Intent(Intent.ACTION_SEND_MULTIPLE).putParcelableArrayListExtra(Intent.EXTRA_STREAM, ArrayList(uris))
    i.type = if (files.size == 1) (files[0].mime ?: mimeOf(files[0].name)) else "*/*"
    i.clipData = ClipData.newRawUri("", uris[0]).apply { uris.drop(1).forEach { addItem(ClipData.Item(it)) } }
    i.addFlags(Intent.FLAG_GRANT_READ_URI_PERMISSION or Intent.FLAG_ACTIVITY_NEW_TASK)
    try {
        if (meshdrop) ctx.startActivity(i.setPackage(MESHDROP_PKG))
        else ctx.startActivity(Intent.createChooser(i, "Send").addFlags(Intent.FLAG_ACTIVITY_NEW_TASK))
    } catch (_: ActivityNotFoundException) {
        state.toast(if (meshdrop) "MeshDrop isn't installed" else "No app can receive these files")
    }
}

/** To Recently deleted when it's on, else gone for good. */
fun deleteEntries(state: AppState, fs: Fs, items: List<Entry>) {
    state.run("Deleting…") {
        val changed = ArrayList<String>()
        for (e in items) {
            if (fs.isLocal && state.prefs.trashDays > 0) state.trash.put(File(e.path)) else fs.delete(e)
            changed += e.path
        }
        if (fs.isLocal) rescan(state.ctx, changed)
        state.toast(if (fs.isLocal && state.prefs.trashDays > 0) "Moved ${items.size} to Recently deleted" else "Deleted ${items.size}")
    }
}

fun paste(state: AppState, dst: Fs, dir: String) {
    val clip = state.clip ?: return
    state.clip = null
    state.run(if (clip.move) "Moving…" else "Copying…") {
        val changed = ArrayList<String>()
        for (e in clip.items) {
            if (clip.move) Transfer.move(clip.fs, e, dst, dir) { state.busy = (if (clip.move) "Moving " else "Copying ") + it }
            else Transfer.copy(clip.fs, e, dst, dir) { state.busy = "Copying $it" }
            changed += e.path
        }
        if (clip.fs.isLocal && clip.move) rescan(state.ctx, changed)
        if (dst.isLocal) rescan(state.ctx, File(dir).listFiles()?.map { it.path } ?: emptyList())
        state.toast("${if (clip.move) "Moved" else "Copied"} ${clip.items.size}")
    }
}

fun compress(state: AppState, items: List<Entry>) {
    state.run("Compressing…") {
        val dir = File(items[0].path).parentFile!!
        val base = if (items.size == 1) items[0].name.substringBeforeLast('.').ifEmpty { items[0].name } else "Archive"
        val out = File(dir, uniqueName("$base.zip") { File(dir, it).exists() })
        Zip.compress(items.map { File(it.path) }, out)
        rescan(state.ctx, listOf(out.path))
        state.toast("Saved ${out.name}")
    }
}

fun extract(state: AppState, e: Entry) {
    state.run("Extracting…") {
        val f = File(e.path)
        val dest = File(f.parentFile, uniqueName(f.name.substringBeforeLast('.')) { File(f.parentFile, it).exists() })
        Zip.extract(f, dest)
        rescan(state.ctx, dest.walkTopDown().map { it.path }.toList())
        state.toast("Extracted to ${dest.name}")
    }
}

fun copyPathsToClipboard(ctx: Context, items: List<Entry>) {
    val cm = ctx.getSystemService(ClipboardManager::class.java)
    cm.setPrimaryClip(ClipData.newPlainText("Paths", items.joinToString("\n") { it.path }))
}
