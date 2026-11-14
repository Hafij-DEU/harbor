@file:OptIn(ExperimentalFoundationApi::class, ExperimentalMaterial3Api::class)

package com.hafij.harborfiles

import android.graphics.Bitmap
import android.media.ThumbnailUtils
import android.util.LruCache
import android.util.Size
import androidx.compose.foundation.ExperimentalFoundationApi
import androidx.compose.foundation.Image
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.combinedClickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.RowScope
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.aspectRatio
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.navigationBarsPadding
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.statusBarsPadding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.grid.GridCells
import androidx.compose.foundation.lazy.grid.GridItemSpan
import androidx.compose.foundation.lazy.grid.LazyVerticalGrid
import androidx.compose.foundation.lazy.grid.items
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.rounded.ArrowBack
import androidx.compose.material.icons.automirrored.rounded.DriveFileMove
import androidx.compose.material.icons.rounded.Android
import androidx.compose.material.icons.rounded.AudioFile
import androidx.compose.material.icons.rounded.Check
import androidx.compose.material.icons.rounded.Close
import androidx.compose.material.icons.rounded.ContentPaste
import androidx.compose.material.icons.rounded.Delete
import androidx.compose.material.icons.rounded.Description
import androidx.compose.material.icons.rounded.Folder
import androidx.compose.material.icons.rounded.FolderZip
import androidx.compose.material.icons.rounded.Image
import androidx.compose.material.icons.rounded.InsertDriveFile
import androidx.compose.material.icons.rounded.MoreVert
import androidx.compose.material.icons.rounded.PlayCircle
import androidx.compose.material.icons.rounded.SelectAll
import androidx.compose.material.icons.rounded.Send
import androidx.compose.material.icons.rounded.Share
import androidx.compose.material.icons.rounded.VideoFile
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.DropdownMenu
import androidx.compose.material3.DropdownMenuItem
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.ListItem
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.ModalBottomSheet
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.produceState
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.ImageBitmap
import androidx.compose.ui.graphics.asImageBitmap
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.layout.ContentScale
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import androidx.core.graphics.drawable.toBitmap
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
import java.io.File

// ---------------------------------------------------------------- thumbnails

object Thumbs {
    private val cache = object : LruCache<String, Bitmap>(40 * 1024 * 1024) {
        override fun sizeOf(key: String, value: Bitmap) = value.byteCount
    }

    fun cached(path: String): Bitmap? = cache.get(path)

    fun load(ctx: android.content.Context, e: Entry): Bitmap? {
        cache.get(e.path)?.let { return it }
        val f = File(e.path)
        val sz = Size(256, 256)
        val b = runCatching {
            when (e.kind) {
                Kind.Image -> ThumbnailUtils.createImageThumbnail(f, sz, null)
                Kind.Video -> ThumbnailUtils.createVideoThumbnail(f, sz, null)
                Kind.Audio -> ThumbnailUtils.createAudioThumbnail(f, sz, null)
                Kind.Apk -> {
                    val pm = ctx.packageManager
                    pm.getPackageArchiveInfo(e.path, 0)?.applicationInfo?.let {
                        it.sourceDir = e.path; it.publicSourceDir = e.path
                        it.loadIcon(pm).toBitmap(128, 128)
                    }
                }
                else -> null
            }
        }.getOrNull() ?: return null
        cache.put(e.path, b)
        return b
    }
}

fun kindIcon(k: Kind): ImageVector = when (k) {
    Kind.Folder -> Icons.Rounded.Folder
    Kind.Image -> Icons.Rounded.Image
    Kind.Video -> Icons.Rounded.VideoFile
    Kind.Audio -> Icons.Rounded.AudioFile
    Kind.Doc -> Icons.Rounded.Description
    Kind.Archive -> Icons.Rounded.FolderZip
    Kind.Apk -> Icons.Rounded.Android
    Kind.Other -> Icons.Rounded.InsertDriveFile
}

/** Picture for a file: real thumbnail for local media/APKs, else a coloured kind badge. */
@Composable
fun Thumb(e: Entry, local: Boolean, size: Dp, modifier: Modifier = Modifier, corner: Dp = 10.dp, fill: Boolean = false) {
    val ctx = LocalContext.current
    val wantsPic = local && e.kind in setOf(Kind.Image, Kind.Video, Kind.Audio, Kind.Apk)
    val bmp: ImageBitmap? by produceState(Thumbs.cached(e.path)?.asImageBitmap(), e.path, wantsPic) {
        if (wantsPic && value == null) value = withContext(Dispatchers.IO) { Thumbs.load(ctx, e) }?.asImageBitmap()
    }
    val box = if (fill) modifier.fillMaxSize() else modifier.size(size)
    val col = kindColor(e.kind)
    Box(box.clip(RoundedCornerShape(corner)).background(col.copy(alpha = 0.14f)), contentAlignment = Alignment.Center) {
        val b = bmp
        if (b != null) {
            val crop = e.kind == Kind.Image || e.kind == Kind.Video || e.kind == Kind.Audio
            Image(b, null, if (crop) Modifier.fillMaxSize() else Modifier.size(size * 0.7f),
                contentScale = if (crop) ContentScale.Crop else ContentScale.Fit)
            if (e.kind == Kind.Video) Icon(Icons.Rounded.PlayCircle, null, Modifier.size(size * 0.35f).align(Alignment.Center), tint = Color.White.copy(alpha = 0.9f))
        } else {
            Icon(kindIcon(e.kind), null, Modifier.size(size * 0.5f), tint = col)
        }
    }
}

// ---------------------------------------------------------------- rows

@Composable
fun FileRow(
    e: Entry, local: Boolean, selected: Boolean?, onClick: () -> Unit, onLongClick: () -> Unit = {},
    detail: String = subtitle(e),
) {
    Row(
        Modifier.fillMaxWidth()
            .combinedClickable(onClick = onClick, onLongClick = onLongClick)
            .padding(horizontal = 16.dp, vertical = 10.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Thumb(e, local, 44.dp)
        Spacer(Modifier.width(14.dp))
        Column(Modifier.weight(1f)) {
            Text(e.name, maxLines = 1, overflow = TextOverflow.Ellipsis, style = MaterialTheme.typography.bodyLarge)
            if (detail.isNotEmpty()) Text(detail, maxLines = 1, style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.onSurfaceVariant)
        }
        if (selected != null) {
            Spacer(Modifier.width(8.dp))
            RoundCheck(selected)
        }
    }
}

@Composable
fun RoundCheck(on: Boolean) {
    val c = MaterialTheme.colorScheme
    Box(
        Modifier.size(22.dp).clip(CircleShape)
            .background(if (on) c.primary else Color.Transparent)
            .border(1.5.dp, if (on) c.primary else c.outline, CircleShape),
        contentAlignment = Alignment.Center,
    ) { if (on) Icon(Icons.Rounded.Check, null, Modifier.size(15.dp), tint = c.onPrimary) }
}

@Composable
fun TopBar(title: String, onBack: (() -> Unit)?, actions: @Composable RowScope.() -> Unit = {}) {
    Row(
        Modifier.fillMaxWidth().statusBarsPadding().height(56.dp).padding(horizontal = 4.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        if (onBack != null) IconButton(onClick = onBack) { Icon(Icons.AutoMirrored.Rounded.ArrowBack, "Back") }
        else Spacer(Modifier.width(16.dp))
        Text(title, style = MaterialTheme.typography.titleLarge, fontWeight = FontWeight.SemiBold, maxLines = 1,
            overflow = TextOverflow.Ellipsis, modifier = Modifier.weight(1f))
        actions()
    }
}

@Composable
fun Empty(text: String, modifier: Modifier = Modifier) {
    Box(modifier.fillMaxWidth().padding(48.dp), contentAlignment = Alignment.Center) {
        Text(text, color = MaterialTheme.colorScheme.onSurfaceVariant)
    }
}

// ---------------------------------------------------------------- the file list every screen uses

/**
 * List (or photo grid) of entries with Mi-style selection: long-press to select,
 * bottom bar Send / Move / Delete / More. [pasteDir] shows the Paste bar when a clip is waiting.
 */
@Composable
fun FilesView(
    state: AppState,
    fs: Fs,
    title: String,
    entries: List<Entry>?,
    error: String? = null,
    grid: Boolean = false,
    pasteDir: String? = null,
    header: @Composable () -> Unit = {},
    actions: @Composable RowScope.() -> Unit = {},
    detail: (Entry) -> String = { subtitle(it) },
    onOpenDir: (Entry) -> Unit = { state.openFolder(fs, it.path) },
    onOpenFile: (Entry) -> Unit = { e -> state.scope.launch { openEntry(state, fs, e) } },
) {
    var sel by remember(fs.key, title) { mutableStateOf(setOf<String>()) }
    val list = entries ?: emptyList()
    val chosen = list.filter { it.path in sel }
    val selecting = sel.isNotEmpty()
    val tap: (Entry) -> Unit = { e ->
        when {
            selecting -> sel = if (e.path in sel) sel - e.path else sel + e.path
            e.isDir -> onOpenDir(e)
            else -> onOpenFile(e)
        }
    }
    val hold: (Entry) -> Unit = { e -> sel = sel + e.path }
    androidx.activity.compose.BackHandler(enabled = selecting) { sel = emptySet() }

    Column(Modifier.fillMaxSize()) {
        if (selecting) {
            TopBar("${sel.size} selected", null) {
                IconButton(onClick = { sel = if (sel.size == list.size) emptySet() else list.map { it.path }.toSet() }) {
                    Icon(Icons.Rounded.SelectAll, "Select all")
                }
                IconButton(onClick = { sel = emptySet() }) { Icon(Icons.Rounded.Close, "Done") }
            }
        } else TopBar(title, { state.pop() }, actions)

        Box(Modifier.weight(1f)) {
            when {
                error != null -> Empty(error)
                entries == null -> Empty("Loading…")
                list.isEmpty() -> Column { header(); Empty("Nothing here") }
                grid -> LazyVerticalGrid(
                    GridCells.Adaptive(100.dp), Modifier.fillMaxSize(),
                    contentPadding = PaddingValues(8.dp, 0.dp, 8.dp, 120.dp),
                    horizontalArrangement = Arrangement.spacedBy(4.dp), verticalArrangement = Arrangement.spacedBy(4.dp),
                ) {
                    item(span = { GridItemSpan(maxLineSpan) }) { header() }
                    items(list, key = { it.path }) { e ->
                        Box(Modifier.aspectRatio(1f).combinedClickable(onClick = { tap(e) }, onLongClick = { hold(e) })) {
                            Thumb(e, fs.isLocal, 100.dp, fill = true, corner = 6.dp)
                            if (selecting) Box(Modifier.align(Alignment.TopEnd).padding(6.dp)) { RoundCheck(e.path in sel) }
                        }
                    }
                }
                else -> LazyColumn(Modifier.fillMaxSize(), contentPadding = PaddingValues(bottom = 120.dp)) {
                    item { header() }
                    items(list, key = { it.path }) { e ->
                        FileRow(e, fs.isLocal, if (selecting) e.path in sel else null, { tap(e) }, { hold(e) }, detail(e))
                    }
                }
            }
            val clip = state.clip
            if (!selecting && clip != null && pasteDir != null) {
                PasteBar(clip, Modifier.align(Alignment.BottomCenter), onCancel = { state.clip = null }) {
                    paste(state, fs, pasteDir)
                }
            }
        }
        if (selecting) SelectionBar(state, fs, chosen) { sel = emptySet() }
    }
}

@Composable
fun PasteBar(clip: Clip, modifier: Modifier, onCancel: () -> Unit, onPaste: () -> Unit) {
    Surface(
        modifier.navigationBarsPadding().padding(16.dp).fillMaxWidth(),
        shape = RoundedCornerShape(20.dp), shadowElevation = 8.dp, color = MaterialTheme.colorScheme.primaryContainer,
    ) {
        Row(Modifier.padding(start = 20.dp, end = 8.dp, top = 6.dp, bottom = 6.dp), verticalAlignment = Alignment.CenterVertically) {
            Text(
                "${if (clip.move) "Move" else "Copy"} ${clip.items.size} item${if (clip.items.size == 1) "" else "s"} here?",
                Modifier.weight(1f), color = MaterialTheme.colorScheme.onPrimaryContainer,
            )
            TextButton(onClick = onCancel) { Text("Cancel") }
            TextButton(onClick = onPaste) { Icon(Icons.Rounded.ContentPaste, null); Spacer(Modifier.width(6.dp)); Text("Paste") }
        }
    }
}

@Composable
private fun BarButton(label: String, icon: ImageVector, enabled: Boolean = true, onClick: () -> Unit) {
    Column(
        Modifier.clip(RoundedCornerShape(12.dp)).clickable(enabled = enabled, onClick = onClick).padding(horizontal = 14.dp, vertical = 6.dp),
        horizontalAlignment = Alignment.CenterHorizontally,
    ) {
        val c = if (enabled) MaterialTheme.colorScheme.onSurface else MaterialTheme.colorScheme.outline
        Icon(icon, null, tint = c)
        Text(label, style = MaterialTheme.typography.labelSmall, color = c)
    }
}

@Composable
fun SelectionBar(state: AppState, fs: Fs, items: List<Entry>, done: () -> Unit) {
    var more by remember { mutableStateOf(false) }
    var send by remember { mutableStateOf(false) }
    var confirmDelete by remember { mutableStateOf(false) }
    var renaming by remember { mutableStateOf<Entry?>(null) }
    var details by remember { mutableStateOf<Entry?>(null) }
    val files = items.filter { !it.isDir }
    Surface(shadowElevation = 8.dp, color = MaterialTheme.colorScheme.surface) {
        Row(Modifier.fillMaxWidth().navigationBarsPadding().padding(vertical = 6.dp), horizontalArrangement = Arrangement.SpaceEvenly) {
            BarButton("Send", Icons.Rounded.Send, files.isNotEmpty()) { send = true }
            BarButton("Move", Icons.AutoMirrored.Rounded.DriveFileMove, items.isNotEmpty()) {
                state.clip = Clip(fs, items, move = true); done(); state.toast("Open a folder and tap Paste")
            }
            BarButton("Delete", Icons.Rounded.Delete, items.isNotEmpty()) { confirmDelete = true }
            Box {
                BarButton("More", Icons.Rounded.MoreVert) { more = true }
                DropdownMenu(more, { more = false }) {
                    DropdownMenuItem({ Text("Copy") }, enabled = items.isNotEmpty(), onClick = {
                        more = false; state.clip = Clip(fs, items, move = false); done(); state.toast("Open a folder and tap Paste")
                    })
                    if (fs.isLocal) DropdownMenuItem({ Text("Copy path") }, onClick = {
                        more = false; copyPathsToClipboard(state.ctx, items); done()
                    })
                    if (fs.isLocal) DropdownMenuItem({ Text("Add to / remove from favourites") }, onClick = {
                        more = false; items.forEach { state.prefs.toggleFavorite(it.path) }; done()
                    })
                    if (items.size == 1) DropdownMenuItem({ Text("Rename") }, onClick = { more = false; renaming = items[0] })
                    if (fs.isLocal && items.isNotEmpty()) DropdownMenuItem({ Text("Compress to zip") }, onClick = {
                        more = false; compress(state, items); done()
                    })
                    if (files.size == 1) DropdownMenuItem({ Text("Open in another app") }, onClick = {
                        more = false; state.scope.launch { openEntry(state, fs, files[0], chooser = true) }
                    })
                    if (items.size == 1) DropdownMenuItem({ Text("Details") }, onClick = { more = false; details = items[0] })
                }
            }
        }
    }
    if (send) SendSheet(state, onDismiss = { send = false }) { meshdrop ->
        send = false
        state.scope.launch { shareEntries(state, fs, files, meshdrop) }
        done()
    }
    if (confirmDelete) {
        val toTrash = fs.isLocal && state.prefs.trashDays > 0
        ConfirmDialog(
            if (toTrash) "Move ${items.size} to Recently deleted?" else "Delete ${items.size} for good?",
            if (toTrash) "You can restore them for ${state.prefs.trashDays} days." else "This can't be undone.",
            "Delete", onDismiss = { confirmDelete = false },
        ) { confirmDelete = false; deleteEntries(state, fs, items); done() }
    }
    renaming?.let { e ->
        NameDialog("Rename", e.name, onDismiss = { renaming = null }) { n ->
            renaming = null
            state.run("Renaming…") {
                fs.rename(e, n)
                if (fs.isLocal) rescan(state.ctx, listOf(e.path, File(File(e.path).parentFile, n).path))
            }
            done()
        }
    }
    details?.let { DetailsDialog(it, fs) { details = null } }
}

@Composable
fun SendSheet(state: AppState, onDismiss: () -> Unit, onPick: (meshdrop: Boolean) -> Unit) {
    val hasMesh = state.prefs.meshdrop && remember { meshdropInstalled(state.ctx) }
    if (!hasMesh) { onPick(false); return }
    ModalBottomSheet(onDismissRequest = onDismiss) {
        Text("Send", Modifier.padding(start = 24.dp, bottom = 8.dp), style = MaterialTheme.typography.titleMedium)
        ListItem(
            headlineContent = { Text("MeshDrop") },
            supportingContent = { Text("To your PC or other devices on the mesh") },
            leadingContent = { Icon(Icons.Rounded.Send, null, tint = MaterialTheme.colorScheme.primary) },
            modifier = Modifier.clickable { onPick(true) },
        )
        ListItem(
            headlineContent = { Text("Other apps") },
            leadingContent = { Icon(Icons.Rounded.Share, null) },
            modifier = Modifier.clickable { onPick(false) },
        )
        Spacer(Modifier.height(24.dp))
    }
}

@Composable
fun ConfirmDialog(title: String, text: String, ok: String, onDismiss: () -> Unit, onOk: () -> Unit) {
    AlertDialog(
        onDismissRequest = onDismiss, title = { Text(title) }, text = { Text(text) },
        confirmButton = { TextButton(onClick = onOk) { Text(ok) } },
        dismissButton = { TextButton(onClick = onDismiss) { Text("Cancel") } },
    )
}

@Composable
fun NameDialog(title: String, initial: String, onDismiss: () -> Unit, onOk: (String) -> Unit) {
    var v by remember { mutableStateOf(initial) }
    val bad = v.isBlank() || '/' in v || v == "." || v == ".."
    AlertDialog(
        onDismissRequest = onDismiss, title = { Text(title) },
        text = { OutlinedTextField(v, { v = it }, singleLine = true, isError = bad && v.isNotEmpty()) },
        confirmButton = { TextButton(onClick = { onOk(v.trim()) }, enabled = !bad) { Text("OK") } },
        dismissButton = { TextButton(onClick = onDismiss) { Text("Cancel") } },
    )
}

@Composable
fun DetailsDialog(e: Entry, fs: Fs, onDismiss: () -> Unit) {
    val rows = buildList {
        add("Name" to e.name)
        add("Where" to if (fs.isLocal) e.path else "${fs.title}: ${e.path}")
        if (!e.isDir) add("Size" to fmtSize(e.size)) else if (e.items >= 0) add("Items" to "${e.items}")
        if (e.modified > 0) add("Modified" to fmtDate(e.modified))
        if (!e.isDir) add("Type" to (e.mime ?: mimeOf(e.name)))
        if (fs.isLocal) add("Source" to sourceOf(e.path))
    }
    AlertDialog(
        onDismissRequest = onDismiss, title = { Text("Details") },
        text = {
            Column {
                rows.forEach { (k, v) ->
                    Text(k, style = MaterialTheme.typography.labelMedium, color = MaterialTheme.colorScheme.onSurfaceVariant)
                    Text(v, style = MaterialTheme.typography.bodyMedium, modifier = Modifier.padding(bottom = 8.dp))
                }
            }
        },
        confirmButton = { TextButton(onClick = onDismiss) { Text("Close") } },
    )
}

/** Rounded card like the Mi file manager's grouped sections. */
@Composable
fun Card(modifier: Modifier = Modifier, content: @Composable () -> Unit) {
    Surface(modifier.fillMaxWidth().padding(horizontal = 14.dp, vertical = 6.dp), shape = RoundedCornerShape(22.dp),
        color = MaterialTheme.colorScheme.surface) { Column(Modifier.padding(vertical = 6.dp)) { content() } }
}

@Composable
fun Divider() = HorizontalDivider(Modifier.padding(start = 74.dp), color = MaterialTheme.colorScheme.outlineVariant.copy(alpha = 0.5f))

val numberKeys = KeyboardOptions(keyboardType = androidx.compose.ui.text.input.KeyboardType.Number)
