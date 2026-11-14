@file:OptIn(ExperimentalFoundationApi::class, ExperimentalMaterial3Api::class)

package com.hafij.harborfiles

import android.content.Context
import android.os.StatFs
import android.os.storage.StorageManager
import android.text.format.DateUtils
import androidx.compose.foundation.ExperimentalFoundationApi
import androidx.compose.foundation.Image
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.combinedClickable
import androidx.compose.foundation.horizontalScroll
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.aspectRatio
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.statusBarsPadding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.rounded.KeyboardArrowRight
import androidx.compose.material.icons.rounded.Add
import androidx.compose.material.icons.rounded.QrCodeScanner
import androidx.compose.material.icons.rounded.Android
import androidx.compose.material.icons.rounded.AudioFile
import androidx.compose.material.icons.rounded.Computer
import androidx.compose.material.icons.rounded.CreateNewFolder
import androidx.compose.material.icons.rounded.Delete
import androidx.compose.material.icons.rounded.Description
import androidx.compose.material.icons.rounded.Download
import androidx.compose.material.icons.rounded.Folder
import androidx.compose.material.icons.rounded.FolderZip
import androidx.compose.material.icons.rounded.Image
import androidx.compose.material.icons.rounded.MoreVert
import androidx.compose.material.icons.rounded.PhoneAndroid
import androidx.compose.material.icons.rounded.SdStorage
import androidx.compose.material.icons.rounded.Search
import androidx.compose.material.icons.rounded.Sort
import androidx.compose.material.icons.rounded.Star
import androidx.compose.material.icons.rounded.VideoFile
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.Button
import androidx.compose.material3.DropdownMenu
import androidx.compose.material3.DropdownMenuItem
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.FilterChip
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.LinearProgressIndicator
import androidx.compose.material3.ListItem
import androidx.compose.material3.ListItemDefaults
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Surface
import androidx.compose.material3.Switch
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.material3.TextField
import androidx.compose.material3.TextFieldDefaults
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.produceState
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.focus.focusRequester
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.ImageBitmap
import androidx.compose.ui.graphics.asImageBitmap
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.layout.ContentScale
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.text.input.PasswordVisualTransformation
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.google.zxing.BarcodeFormat
import com.google.zxing.qrcode.QRCodeWriter
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.delay
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
import java.io.File
import java.text.DateFormat
import java.text.SimpleDateFormat
import java.util.Date
import java.util.Locale
import java.util.UUID

// ---------------------------------------------------------------- Recent

data class RecentGroup(val day: String, val source: String, val items: List<Entry>)

fun dayLabel(ms: Long): String = when {
    DateUtils.isToday(ms) -> "Today"
    DateUtils.isToday(ms + DateUtils.DAY_IN_MILLIS) -> "Yesterday"
    else -> DateFormat.getDateInstance(DateFormat.MEDIUM).format(Date(ms))
}

/** Same day + same source = one card, in newest-first order. */
fun groupRecent(list: List<Entry>): List<RecentGroup> {
    val dayKey = SimpleDateFormat("yyyyMMdd", Locale.ROOT)
    val m = LinkedHashMap<String, MutableList<Entry>>()
    val meta = HashMap<String, Pair<String, String>>()
    for (e in list) {
        val src = sourceOf(e.path)
        val k = dayKey.format(Date(e.modified)) + "|" + src
        m.getOrPut(k) { meta[k] = dayLabel(e.modified) to src; ArrayList() }.add(e)
    }
    return m.map { (k, v) -> RecentGroup(meta[k]!!.first, meta[k]!!.second, v) }
}

private fun isMedia(e: Entry) = e.kind == Kind.Image || e.kind == Kind.Video

@Composable
fun RecentScreen(state: AppState) {
    val groups by produceState<List<RecentGroup>?>(null, state.rev, state.prefs.hideScreenshots) {
        value = withContext(Dispatchers.IO) {
            runCatching {
                Media.recent(state.ctx).filter { !it.isDir && !(state.prefs.hideScreenshots && sourceOf(it.path) == "Screenshots") }
            }.getOrDefault(emptyList()).let(::groupRecent)
        }
    }
    LazyColumn(Modifier.fillMaxSize().statusBarsPadding(), contentPadding = PaddingValues(bottom = 110.dp)) {
        item { BigTitle("Recent") }
        val g = groups
        when {
            g == null -> item { Empty("Loading…") }
            g.isEmpty() -> item { Empty("No recent files") }
            else -> {
                var lastDay = ""
                for (grp in g) {
                    if (grp.day != lastDay) {
                        lastDay = grp.day
                        item(key = "d" + grp.day + grp.source) {
                            Text(grp.day, Modifier.padding(start = 22.dp, top = 12.dp, bottom = 2.dp),
                                style = MaterialTheme.typography.titleSmall, color = MaterialTheme.colorScheme.onSurfaceVariant)
                        }
                    }
                    item(key = "g" + grp.day + grp.source) { RecentCard(state, grp) }
                }
            }
        }
    }
}

@Composable
private fun RecentCard(state: AppState, g: RecentGroup) {
    val media = g.items.filter(::isMedia)
    val other = g.items.filterNot(::isMedia)
    val time = DateFormat.getTimeInstance(DateFormat.SHORT).format(Date(g.items.first().modified))
    val openAll = { state.push(Screen.Group("${g.source} · ${g.day}", g.items)) }
    Card {
        Row(Modifier.fillMaxWidth().clickable(onClick = openAll).padding(horizontal = 16.dp, vertical = 8.dp), verticalAlignment = Alignment.CenterVertically) {
            Text(g.source, fontWeight = FontWeight.SemiBold, modifier = Modifier.weight(1f))
            Text("$time · ${g.items.size}", style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.onSurfaceVariant)
            Icon(Icons.AutoMirrored.Rounded.KeyboardArrowRight, null, tint = MaterialTheme.colorScheme.onSurfaceVariant)
        }
        if (media.isNotEmpty()) {
            val shown = media.take(8)
            Column(Modifier.padding(horizontal = 12.dp), verticalArrangement = Arrangement.spacedBy(4.dp)) {
                shown.chunked(4).forEachIndexed { row, chunk ->
                    Row(horizontalArrangement = Arrangement.spacedBy(4.dp)) {
                        for (i in 0 until 4) {
                            val e = chunk.getOrNull(i)
                            Box(Modifier.weight(1f).aspectRatio(1f)) {
                                if (e != null) {
                                    val idx = row * 4 + i
                                    val extra = media.size - shown.size
                                    Box(Modifier.fillMaxSize().clickable {
                                        if (idx == shown.size - 1 && extra > 0) openAll()
                                        else state.scope.launch { openEntry(state, state.local, e) }
                                    }) {
                                        Thumb(e, true, 80.dp, fill = true, corner = 8.dp)
                                        if (idx == shown.size - 1 && extra > 0) Box(
                                            Modifier.fillMaxSize().clip(RoundedCornerShape(8.dp)).background(Color.Black.copy(alpha = 0.45f)),
                                            contentAlignment = Alignment.Center,
                                        ) { Text("+$extra", color = Color.White, fontSize = 20.sp, fontWeight = FontWeight.SemiBold) }
                                    }
                                }
                            }
                        }
                    }
                }
            }
            Spacer(Modifier.height(6.dp))
        }
        other.take(3).forEach { e ->
            FileRow(e, true, null, { state.scope.launch { openEntry(state, state.local, e) } }, openAll)
        }
        if (other.size > 3) Text("+${other.size - 3} more", Modifier.clickable(onClick = openAll).padding(start = 74.dp, bottom = 8.dp, top = 2.dp),
            color = MaterialTheme.colorScheme.primary, style = MaterialTheme.typography.bodySmall)
    }
}

@Composable
fun GroupScreen(state: AppState, s: Screen.Group) {
    val items = remember(s, state.rev) { s.items.filter { File(it.path).exists() } }
    FilesView(state, state.local, s.title, items, grid = items.isNotEmpty() && items.all(::isMedia))
}

// ---------------------------------------------------------------- Browse

private fun catIcon(c: Cat): Pair<ImageVector, Kind> = when (c) {
    Cat.Docs -> Icons.Rounded.Description to Kind.Doc
    Cat.Images -> Icons.Rounded.Image to Kind.Image
    Cat.Videos -> Icons.Rounded.VideoFile to Kind.Video
    Cat.Music -> Icons.Rounded.AudioFile to Kind.Audio
    Cat.Archives -> Icons.Rounded.FolderZip to Kind.Archive
    Cat.Apks -> Icons.Rounded.Android to Kind.Apk
}

data class Volume(val title: String, val path: String, val free: Long, val total: Long)

fun volumes(ctx: Context): List<Volume> {
    val out = ArrayList<Volume>()
    val sm = ctx.getSystemService(StorageManager::class.java)
    for (v in sm.storageVolumes) {
        val dir = (if (v.isPrimary) File(PHONE_ROOT) else v.directory) ?: continue
        val st = runCatching { StatFs(dir.path) }.getOrNull() ?: continue
        out += Volume(if (v.isPrimary) "Internal storage" else v.getDescription(ctx), dir.path, st.availableBytes, st.totalBytes)
    }
    return out
}

@Composable
fun BrowseScreen(state: AppState) {
    val counts by produceState<Map<Cat, Int>>(emptyMap(), state.rev) {
        value = withContext(Dispatchers.IO) { Cat.entries.associateWith { runCatching { Media.count(state.ctx, it) }.getOrDefault(0) } }
    }
    val vols by produceState<List<Volume>>(emptyList(), state.rev) { value = withContext(Dispatchers.IO) { volumes(state.ctx) } }
    val sources = remember(state.rev) { sourceDirs() }
    var menu by remember { mutableStateOf(false) }
    Column(Modifier.fillMaxSize().statusBarsPadding().verticalScroll(rememberScrollState()).padding(bottom = 110.dp)) {
        BigTitle("Browse") {
            Box {
                IconButton(onClick = { menu = true }) { Icon(Icons.Rounded.MoreVert, "Menu") }
                DropdownMenu(menu, { menu = false }) {
                    DropdownMenuItem({ Text("Settings") }, onClick = { menu = false; state.push(Screen.Settings) })
                }
            }
        }
        Card {
            Cat.entries.chunked(3).forEach { row ->
                Row(Modifier.fillMaxWidth().padding(horizontal = 8.dp)) {
                    row.forEach { c ->
                        val (ic, k) = catIcon(c)
                        Column(
                            Modifier.weight(1f).clip(RoundedCornerShape(16.dp)).clickable { state.push(Screen.Category(c)) }.padding(vertical = 12.dp),
                            horizontalAlignment = Alignment.CenterHorizontally,
                        ) {
                            Box(Modifier.size(48.dp).clip(RoundedCornerShape(14.dp)).background(kindColor(k).copy(alpha = 0.15f)), contentAlignment = Alignment.Center) {
                                Icon(ic, null, tint = kindColor(k))
                            }
                            Spacer(Modifier.height(6.dp))
                            Text(c.label, style = MaterialTheme.typography.bodyMedium)
                            Text(counts[c]?.toString() ?: "–", style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.onSurfaceVariant)
                        }
                    }
                }
            }
        }
        Card {
            NavRow("Downloads", Icons.Rounded.Download) {
                val d = File(PHONE_ROOT, "Download"); state.openFolder(state.local, d.path, d.path, "Downloads")
            }
            Divider()
            NavRow("Favourites", Icons.Rounded.Star, "${state.prefs.favorites.size}") { state.push(Screen.Favorites) }
            Divider()
            NavRow("Recently deleted", Icons.Rounded.Delete) { state.push(Screen.Trash) }
        }
        vols.forEach { v ->
            Card {
                Row(
                    Modifier.fillMaxWidth().clickable { state.openFolder(state.local, v.path, v.path, v.title) }.padding(16.dp),
                    verticalAlignment = Alignment.CenterVertically,
                ) {
                    Icon(if (v.path == PHONE_ROOT) Icons.Rounded.PhoneAndroid else Icons.Rounded.SdStorage, null, tint = MaterialTheme.colorScheme.primary)
                    Spacer(Modifier.width(16.dp))
                    Column(Modifier.weight(1f)) {
                        Text(v.title, fontWeight = FontWeight.SemiBold)
                        Spacer(Modifier.height(6.dp))
                        LinearProgressIndicator(
                            progress = { if (v.total > 0) 1f - v.free.toFloat() / v.total else 0f },
                            Modifier.fillMaxWidth().height(6.dp).clip(CircleShape),
                        )
                        Spacer(Modifier.height(4.dp))
                        Text("${fmtSize(v.total - v.free)} used of ${fmtSize(v.total)}", style = MaterialTheme.typography.bodySmall,
                            color = MaterialTheme.colorScheme.onSurfaceVariant)
                    }
                    Icon(Icons.AutoMirrored.Rounded.KeyboardArrowRight, null)
                }
            }
        }
        if (sources.isNotEmpty()) {
            Text("Sources", Modifier.padding(start = 22.dp, top = 12.dp, bottom = 2.dp), style = MaterialTheme.typography.titleSmall,
                color = MaterialTheme.colorScheme.onSurfaceVariant)
            Card {
                sources.forEachIndexed { i, (n, f) ->
                    if (i > 0) Divider()
                    NavRow(n, Icons.Rounded.Folder) { state.openFolder(state.local, f.path, f.path, n) }
                }
            }
        }
    }
}

@Composable
fun NavRow(label: String, icon: ImageVector, trailing: String = "", onClick: () -> Unit) {
    Row(Modifier.fillMaxWidth().clickable(onClick = onClick).padding(horizontal = 16.dp, vertical = 14.dp), verticalAlignment = Alignment.CenterVertically) {
        Box(Modifier.size(40.dp).clip(RoundedCornerShape(12.dp)).background(MaterialTheme.colorScheme.primary.copy(alpha = 0.12f)), contentAlignment = Alignment.Center) {
            Icon(icon, null, tint = MaterialTheme.colorScheme.primary)
        }
        Spacer(Modifier.width(18.dp))
        Text(label, Modifier.weight(1f))
        if (trailing.isNotEmpty()) Text(trailing, color = MaterialTheme.colorScheme.onSurfaceVariant)
        Icon(Icons.AutoMirrored.Rounded.KeyboardArrowRight, null, tint = MaterialTheme.colorScheme.onSurfaceVariant)
    }
}

@Composable
fun CategoryScreen(state: AppState, c: Cat) {
    val items by produceState<List<Entry>?>(null, c, state.rev) {
        value = withContext(Dispatchers.IO) { runCatching { Media.category(state.ctx, c) }.getOrDefault(emptyList()) }
    }
    FilesView(state, state.local, c.label, items, grid = c == Cat.Images || c == Cat.Videos)
}

@Composable
fun FavoritesScreen(state: AppState) {
    val items = remember(state.prefs.favorites, state.rev) {
        state.prefs.favorites.map(::File).filter { it.exists() }.map {
            Entry(it.name, it.path, it.isDirectory, if (it.isFile) it.length() else 0, it.lastModified())
        }.sortedBy { it.name.lowercase() }
    }
    FilesView(state, state.local, "Favourites", items, onOpenDir = { state.openFolder(state.local, it.path, it.path, it.name) })
}

// ---------------------------------------------------------------- Folder

private val SORTS = listOf("Name", "Date", "Size", "Type")

fun sortEntries(l: List<Entry>, by: Int, desc: Boolean): List<Entry> {
    val c: Comparator<Entry> = when (by) {
        1 -> compareBy { it.modified }
        2 -> compareBy { it.size }
        3 -> compareBy<Entry> { ext(it.name) }.thenBy { it.name.lowercase() }
        else -> compareBy { it.name.lowercase() }
    }
    val files = l.sortedWith(if (desc) c.reversed() else c)
    return files.filter { it.isDir } + files.filter { !it.isDir }
}

@Composable
fun FolderScreen(state: AppState, s: Screen.Folder) {
    val fs = s.fs
    var nonce by remember { mutableStateOf(0) }
    var err by remember(s) { mutableStateOf<String?>(null) }
    val raw by produceState<List<Entry>?>(null, s, state.rev, nonce, state.prefs.showHidden, state.prefs.hideSystem) {
        value = withContext(Dispatchers.IO) {
            try { err = null; fs.list(s.path) } catch (e: Exception) { err = e.message ?: "Can't open this folder"; emptyList() }
        }
    }
    val items = raw?.let { sortEntries(it, state.prefs.sort, state.prefs.sortDesc) }
    var sortMenu by remember { mutableStateOf(false) }
    var newFolder by remember { mutableStateOf(false) }
    var zip by remember { mutableStateOf<Entry?>(null) }
    val title = if (s.path.trimEnd('/') == s.base.trimEnd('/')) s.baseTitle else fs.nameOf(s.path)
    FilesView(
        state, fs, title, items, err, pasteDir = s.path,
        header = { Crumbs(state, s) },
        actions = {
            IconButton(onClick = { newFolder = true }) { Icon(Icons.Rounded.CreateNewFolder, "New folder") }
            Box {
                IconButton(onClick = { sortMenu = true }) { Icon(Icons.Rounded.Sort, "Sort") }
                DropdownMenu(sortMenu, { sortMenu = false }) {
                    SORTS.forEachIndexed { i, n ->
                        DropdownMenuItem({ Text(n + if (state.prefs.sort == i) (if (state.prefs.sortDesc) "  ↓" else "  ↑") else "") }, onClick = {
                            if (state.prefs.sort == i) state.prefs.sortDesc = !state.prefs.sortDesc else state.prefs.sort = i
                            sortMenu = false
                        })
                    }
                }
            }
        },
        onOpenDir = { state.push(Screen.Folder(fs, it.path, s.base, s.baseTitle)) },
        onOpenFile = { e ->
            if (fs.isLocal && e.kind == Kind.Archive && ext(e.name) == "zip") zip = e
            else state.scope.launch { openEntry(state, fs, e) }
        },
    )
    if (newFolder) NameDialog("New folder", "", onDismiss = { newFolder = false }) { n ->
        newFolder = false; state.run("Creating…") { fs.mkdir(s.path, n) }
    }
    zip?.let { e ->
        AlertDialog(
            onDismissRequest = { zip = null }, title = { Text(e.name) }, text = { Text(fmtSize(e.size)) },
            confirmButton = { TextButton(onClick = { zip = null; extract(state, e) }) { Text("Extract here") } },
            dismissButton = { TextButton(onClick = { zip = null; state.scope.launch { openEntry(state, fs, e) } }) { Text("Open with…") } },
        )
    }
}

@Composable
private fun Crumbs(state: AppState, s: Screen.Folder) {
    val base = s.base.trimEnd('/')
    val rel = s.path.trimEnd('/').removePrefix(base).trim('/')
    val parts = if (rel.isEmpty()) emptyList() else rel.split('/')
    Row(Modifier.fillMaxWidth().horizontalScroll(rememberScrollState()).padding(horizontal = 12.dp, vertical = 4.dp), verticalAlignment = Alignment.CenterVertically) {
        val go = { depth: Int ->
            val target = if (depth == 0) s.base else base + "/" + parts.take(depth).joinToString("/")
            // pop back to an existing screen for that folder if it's on the stack, else push
            val idx = state.stack.indexOfLast { it is Screen.Folder && it.fs.key == s.fs.key && it.path.trimEnd('/') == target.trimEnd('/') }
            if (idx >= 0) while (state.stack.size > idx + 1) state.pop()
            else state.push(Screen.Folder(s.fs, target, s.base, s.baseTitle))
        }
        CrumbChip(s.baseTitle, parts.isEmpty()) { go(0) }
        parts.forEachIndexed { i, p ->
            Icon(Icons.AutoMirrored.Rounded.KeyboardArrowRight, null, Modifier.size(16.dp), tint = MaterialTheme.colorScheme.onSurfaceVariant)
            CrumbChip(p, i == parts.lastIndex) { go(i + 1) }
        }
    }
}

@Composable
private fun CrumbChip(text: String, current: Boolean, onClick: () -> Unit) {
    Text(
        text, maxLines = 1,
        modifier = Modifier.clip(CircleShape).clickable(enabled = !current, onClick = onClick).padding(horizontal = 8.dp, vertical = 4.dp),
        style = MaterialTheme.typography.bodySmall,
        color = if (current) MaterialTheme.colorScheme.onSurface else MaterialTheme.colorScheme.primary,
        fontWeight = if (current) FontWeight.SemiBold else FontWeight.Normal,
    )
}

// ---------------------------------------------------------------- Search

@Composable
fun SearchScreen(state: AppState) {
    var q by remember { mutableStateOf("") }
    var cat by remember { mutableStateOf<Cat?>(null) }
    var res by remember { mutableStateOf<List<Entry>?>(emptyList()) }
    val focus = remember { FocusRequester() }
    LaunchedEffect(Unit) { runCatching { focus.requestFocus() } }
    LaunchedEffect(q, cat, state.rev) {
        if (q.isBlank()) { res = emptyList(); return@LaunchedEffect }
        delay(250)
        res = withContext(Dispatchers.IO) { runCatching { Media.search(state.ctx, q.trim(), cat) }.getOrDefault(emptyList()) }
    }
    FilesView(
        state, state.local, "Search", res,
        header = {
            Column {
                TextField(
                    q, { q = it }, Modifier.fillMaxWidth().padding(horizontal = 14.dp).focusRequester(focus),
                    placeholder = { Text("Search files on this phone") }, singleLine = true,
                    leadingIcon = { Icon(Icons.Rounded.Search, null) },
                    shape = CircleShape,
                    colors = TextFieldDefaults.colors(focusedIndicatorColor = Color.Transparent, unfocusedIndicatorColor = Color.Transparent),
                )
                Row(Modifier.horizontalScroll(rememberScrollState()).padding(horizontal = 14.dp, vertical = 6.dp), horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                    FilterChip(cat == null, { cat = null }, { Text("All") })
                    Cat.entries.forEach { c -> FilterChip(cat == c, { cat = c }, { Text(c.label) }) }
                }
            }
        },
        detail = { e -> subtitle(e) + "  ·  " + sourceOf(e.path) },
        onOpenDir = { state.openFolder(state.local, it.path, it.path, it.name) },
    )
}

// ---------------------------------------------------------------- Recently deleted

@Composable
fun TrashScreen(state: AppState) {
    val trash = state.trash
    val items by produceState<List<Trash.Item>?>(null, state.rev) { value = withContext(Dispatchers.IO) { runCatching { trash.list() }.getOrDefault(emptyList()) } }
    var pick by remember { mutableStateOf<Trash.Item?>(null) }
    var empty by remember { mutableStateOf(false) }
    val days = state.prefs.trashDays
    Column(Modifier.fillMaxSize()) {
        TopBar("Recently deleted", { state.pop() }) {
            if (!items.isNullOrEmpty()) TextButton(onClick = { empty = true }) { Text("Empty") }
        }
        val l = items
        when {
            l == null -> Empty("Loading…")
            l.isEmpty() -> Empty(if (days > 0) "Deleted files stay here for $days days" else "Recently deleted is off in Settings")
            else -> LazyColumn(contentPadding = PaddingValues(bottom = 40.dp)) {
                items(l, key = { it.id }) { t ->
                    val e = Entry(t.name, File(trash.dir, t.id).path, t.isDir, t.size, t.deletedAt)
                    val left = trash.daysLeft(t, days)
                    FileRow(e, true, null, { pick = t }, { pick = t },
                        detail = (if (t.isDir) "Folder" else fmtSize(t.size)) + "  ·  " + (if (days > 0) "$left days left" else "kept"))
                }
            }
        }
    }
    pick?.let { t ->
        AlertDialog(
            onDismissRequest = { pick = null }, title = { Text(t.name) },
            text = { Text("From ${t.origPath.removePrefix(PHONE_ROOT).ifEmpty { "/" }}") },
            confirmButton = { TextButton(onClick = {
                pick = null; state.run("Restoring…") { val f = trash.restore(t); rescan(state.ctx, listOf(f.path)); state.toast("Restored ${f.name}") }
            }) { Text("Restore") } },
            dismissButton = { TextButton(onClick = { pick = null; state.run("Deleting…") { trash.purge(t) } }) { Text("Delete now") } },
        )
    }
    if (empty) ConfirmDialog("Empty Recently deleted?", "Everything here is deleted for good.", "Empty", { empty = false }) {
        empty = false; state.run("Emptying…") { trash.list().forEach { trash.purge(it) } }
    }
}

// ---------------------------------------------------------------- Settings

@Composable
fun SettingsScreen(state: AppState) {
    val p = state.prefs
    val version = remember { runCatching { state.ctx.packageManager.getPackageInfo(state.ctx.packageName, 0).versionName }.getOrNull() ?: "" }
    Column(Modifier.fillMaxSize()) {
        TopBar("Settings", { state.pop() })
        Column(Modifier.verticalScroll(rememberScrollState()).padding(bottom = 40.dp)) {
            Section("Files")
            Card {
                SwitchRow("Show hidden files", "Names starting with a dot", p.showHidden) { p.showHidden = it }
                SwitchRow("Hide system folders", "Android, MIUI and LOST.DIR at the top level", p.hideSystem) { p.hideSystem = it }
                SwitchRow("Hide screenshots in Recent", null, p.hideScreenshots) { p.hideScreenshots = it }
            }
            Section("Recently deleted")
            Card {
                ChipRow("Keep deleted files", listOf(0 to "Off", 7 to "7 days", 15 to "15", 30 to "30", 60 to "60"), p.trashDays) { p.trashDays = it }
            }
            Section("Look")
            Card {
                ChipRow("Theme", listOf(0 to "System", 1 to "Light", 2 to "Dark"), p.theme) { p.theme = it }
                Text("Accent", Modifier.padding(start = 16.dp, top = 8.dp), style = MaterialTheme.typography.bodyMedium)
                Row(Modifier.padding(16.dp), horizontalArrangement = Arrangement.spacedBy(14.dp)) {
                    ACCENTS.forEachIndexed { i, (_, c) ->
                        Box(
                            Modifier.size(34.dp).clip(CircleShape).background(c).clickable { p.accent = i }
                                .border(3.dp, if (p.accent == i) MaterialTheme.colorScheme.onSurface else Color.Transparent, CircleShape),
                        )
                    }
                }
            }
            Section("Share this phone")
            Card {
                var port by remember { mutableStateOf(p.ftpPort.toString()) }
                OutlinedTextField(
                    port, { v -> port = v.filter(Char::isDigit).take(5); port.toIntOrNull()?.takeIf { it in 1024..65535 }?.let { p.ftpPort = it } },
                    Modifier.fillMaxWidth().padding(horizontal = 16.dp, vertical = 8.dp), label = { Text("FTP port (1024–65535)") },
                    singleLine = true, keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Number),
                )
                SwitchRow("Read-only", "The PC can download but not change files", p.readOnly) { p.readOnly = it }
                ChipRow("Stop when idle", listOf(0 to "Never", 5 to "5 min", 15 to "15", 30 to "30", 60 to "60"), p.idleMin) { p.idleMin = it }
            }
            Section("MeshDrop")
            Card {
                SwitchRow("Show MeshDrop in Send", if (meshdropInstalled(state.ctx)) "MeshDrop is installed" else "MeshDrop isn't installed", p.meshdrop) { p.meshdrop = it }
            }
            Section("About")
            Card {
                ListItem(
                    headlineContent = { Text("Harbor Files $version") },
                    supportingContent = { Text("Part of Harbor. Your files stay on your phone unless you send or share them.") },
                    colors = ListItemDefaults.colors(containerColor = Color.Transparent),
                )
            }
        }
    }
}

@Composable
private fun Section(t: String) = Text(t, Modifier.padding(start = 22.dp, top = 14.dp, bottom = 2.dp), style = MaterialTheme.typography.titleSmall,
    color = MaterialTheme.colorScheme.primary)

@Composable
fun SwitchRow(title: String, sub: String?, on: Boolean, set: (Boolean) -> Unit) {
    ListItem(
        headlineContent = { Text(title) },
        supportingContent = sub?.let { { Text(it) } },
        trailingContent = { Switch(on, set) },
        modifier = Modifier.clickable { set(!on) },
        colors = ListItemDefaults.colors(containerColor = Color.Transparent),
    )
}

@Composable
private fun ChipRow(title: String, opts: List<Pair<Int, String>>, cur: Int, set: (Int) -> Unit) {
    Text(title, Modifier.padding(start = 16.dp, top = 10.dp), style = MaterialTheme.typography.bodyMedium)
    Row(Modifier.horizontalScroll(rememberScrollState()).padding(horizontal = 16.dp, vertical = 6.dp), horizontalArrangement = Arrangement.spacedBy(8.dp)) {
        opts.forEach { (v, l) -> FilterChip(cur == v, { set(v) }, { Text(l) }) }
    }
}

// ---------------------------------------------------------------- Connect

fun qrBitmap(text: String, px: Int = 512): ImageBitmap {
    val m = QRCodeWriter().encode(text, BarcodeFormat.QR_CODE, px, px, mapOf(com.google.zxing.EncodeHintType.MARGIN to 1))
    val pixels = IntArray(px * px) { i -> if (m.get(i % px, i / px)) 0xFF000000.toInt() else 0xFFFFFFFF.toInt() }
    return android.graphics.Bitmap.createBitmap(pixels, px, px, android.graphics.Bitmap.Config.ARGB_8888).asImageBitmap()
}

@Composable
fun ConnectScreen(state: AppState) {
    val info by ServerService.state
    val err by ServerService.error
    var editing by remember { mutableStateOf<Server?>(null) }
    var adding by remember { mutableStateOf(false) }
    var removing by remember { mutableStateOf<Server?>(null) }
    Column(Modifier.fillMaxSize().statusBarsPadding().verticalScroll(rememberScrollState()).padding(bottom = 110.dp)) {
        BigTitle("Connect")
        Section("This phone")
        Card {
            Column(Modifier.padding(16.dp)) {
                val i = info
                if (i == null) {
                    Text("Let your PC open this phone over Wi-Fi.", fontWeight = FontWeight.SemiBold)
                    Spacer(Modifier.height(4.dp))
                    Text("Starts an FTP server with a one-time PIN. Open it from Harbor on the PC (Other locations), or any FTP app.",
                        style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.onSurfaceVariant)
                    err?.let { Spacer(Modifier.height(8.dp)); Text(it, color = MaterialTheme.colorScheme.error, style = MaterialTheme.typography.bodySmall) }
                    Spacer(Modifier.height(12.dp))
                    Button(onClick = { ServerService.error.value = null; ServerService.start(state.ctx) }) { Text("Start sharing") }
                } else {
                    val qr = remember(i.qrText) { qrBitmap(i.qrText) }
                    Row(verticalAlignment = Alignment.CenterVertically) {
                        Column(Modifier.weight(1f)) {
                            Text("Sharing on Wi-Fi", fontWeight = FontWeight.SemiBold, color = MaterialTheme.colorScheme.primary)
                            Spacer(Modifier.height(8.dp))
                            Text(i.url, fontFamily = FontFamily.Monospace, style = MaterialTheme.typography.bodyLarge)
                            Text("User: harbor", style = MaterialTheme.typography.bodySmall)
                            Spacer(Modifier.height(6.dp))
                            Text("PIN", style = MaterialTheme.typography.labelSmall, color = MaterialTheme.colorScheme.onSurfaceVariant)
                            Text(i.pin.chunked(3).joinToString(" "), fontSize = 28.sp, fontWeight = FontWeight.Bold, fontFamily = FontFamily.Monospace)
                            if (state.prefs.readOnly) Text("Read-only", style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.onSurfaceVariant)
                        }
                        Image(qr, "QR code", Modifier.size(112.dp).clip(RoundedCornerShape(8.dp)), contentScale = ContentScale.Fit)
                    }
                    Spacer(Modifier.height(12.dp))
                    OutlinedButton(onClick = { ServerService.stop(state.ctx) }) { Text("Stop") }
                }
            }
        }
        Row(Modifier.padding(start = 22.dp, end = 8.dp, top = 14.dp), verticalAlignment = Alignment.CenterVertically) {
            Text("Computers & servers", Modifier.weight(1f), style = MaterialTheme.typography.titleSmall, color = MaterialTheme.colorScheme.primary)
            IconButton(onClick = { scanQr(state) }) { Icon(Icons.Rounded.QrCodeScanner, "Scan QR") }
            IconButton(onClick = { adding = true }) { Icon(Icons.Rounded.Add, "Add") }
        }
        val servers = state.prefs.servers
        Card {
            if (servers.isEmpty()) {
                Text("Add your PC: on the PC open Harbor Files › Share over Wi-Fi, then scan its QR code, or tap + and type the address and PIN.",
                    Modifier.padding(16.dp), style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.onSurfaceVariant)
            }
            servers.forEachIndexed { idx, s ->
                if (idx > 0) Divider()
                var menu by remember { mutableStateOf(false) }
                Row(
                    Modifier.fillMaxWidth().combinedClickable(onClick = {
                        val fs = fsFor(s, state.prefs.secret(s.id))
                        state.openFolder(fs, fs.root, fs.root, s.name)
                    }, onLongClick = { menu = true }).padding(horizontal = 16.dp, vertical = 12.dp),
                    verticalAlignment = Alignment.CenterVertically,
                ) {
                    Box(Modifier.size(40.dp).clip(RoundedCornerShape(12.dp)).background(MaterialTheme.colorScheme.primary.copy(alpha = 0.12f)), contentAlignment = Alignment.Center) {
                        Icon(Icons.Rounded.Computer, null, tint = MaterialTheme.colorScheme.primary)
                    }
                    Spacer(Modifier.width(18.dp))
                    Column(Modifier.weight(1f)) {
                        Text(s.name)
                        Text("${if (s.type == "ftp") "FTP" else "Harbor (WebDAV)"} · ${s.host}:${s.port}", style = MaterialTheme.typography.bodySmall,
                            color = MaterialTheme.colorScheme.onSurfaceVariant)
                    }
                    Box {
                        IconButton(onClick = { menu = true }) { Icon(Icons.Rounded.MoreVert, "More") }
                        DropdownMenu(menu, { menu = false }) {
                            DropdownMenuItem({ Text("Edit") }, onClick = { menu = false; editing = s })
                            DropdownMenuItem({ Text("Remove") }, onClick = { menu = false; removing = s })
                        }
                    }
                }
            }
        }
    }
    if (adding || editing != null) ServerDialog(state, editing) { adding = false; editing = null }
    removing?.let { s ->
        ConfirmDialog("Remove ${s.name}?", "Only the saved connection is removed.", "Remove", { removing = null }) {
            state.prefs.removeServer(s.id); removing = null
        }
    }
}

/** Scans a Harbor QR code with Google's code scanner (no camera permission), saves the server and opens it. */
private fun scanQr(state: AppState) {
    val opts = com.google.mlkit.vision.codescanner.GmsBarcodeScannerOptions.Builder()
        .setBarcodeFormats(com.google.mlkit.vision.barcode.common.Barcode.FORMAT_QR_CODE).build()
    com.google.mlkit.vision.codescanner.GmsBarcodeScanning.getClient(state.ctx, opts).startScan()
        .addOnSuccessListener { code ->
            val q = code.rawValue?.let(::parseConnectQr)
            if (q == null) { state.toast("Not a Harbor QR code"); return@addOnSuccessListener }
            val old = state.prefs.servers.firstOrNull { it.type == q.type && it.host == q.host && it.port == q.port }
            val s = Server(old?.id ?: UUID.randomUUID().toString(), q.type,
                old?.name ?: if (q.type == "dav") "PC ${q.host}" else "FTP ${q.host}", q.host, q.port, q.user, q.path)
            state.prefs.saveServer(s)
            if (q.secret != null) state.prefs.setSecret(s.id, q.secret)
            val fs = fsFor(s, state.prefs.secret(s.id))
            state.openFolder(fs, fs.root, fs.root, s.name)
        }
        .addOnFailureListener { e -> state.toast("Scanner not available: ${e.message ?: "try + instead"}") }
}

@Composable
private fun ServerDialog(state: AppState, old: Server?, close: () -> Unit) {
    var type by remember { mutableStateOf(old?.type ?: "dav") }
    var name by remember { mutableStateOf(old?.name ?: "") }
    var host by remember { mutableStateOf(old?.host ?: "") }
    var port by remember { mutableStateOf(old?.port?.toString() ?: "8765") }
    var user by remember { mutableStateOf(old?.user ?: "harbor") }
    var path by remember { mutableStateOf(old?.path ?: "/dav/") }
    var pass by remember { mutableStateOf("") }
    // Accept a pasted address like http://192.168.1.5:8765/ or ftp://ip:port
    fun parseHost(v: String) {
        val m = Regex("""^(?:(https?|ftp)://)?([^/:]+)(?::(\d+))?(/.*)?$""").find(v.trim())
        if (m == null || m.groupValues[1].isEmpty()) { host = v.trim(); return }
        if (m.groupValues[1] == "ftp") type = "ftp" else type = "dav"
        host = m.groupValues[2]
        if (m.groupValues[3].isNotEmpty()) port = m.groupValues[3]
    }
    val ok = host.isNotBlank() && port.toIntOrNull() in 1..65535
    AlertDialog(
        onDismissRequest = close,
        title = { Text(if (old == null) "Add computer" else "Edit ${old.name}") },
        text = {
            Column(Modifier.verticalScroll(rememberScrollState())) {
                Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                    FilterChip(type == "dav", { type = "dav"; if (port == "2121") port = "8765" }, { Text("Harbor PC") })
                    FilterChip(type == "ftp", { type = "ftp"; if (port == "8765") port = "2121" }, { Text("FTP") })
                }
                OutlinedTextField(host, ::parseHost, label = { Text("Address (IP or http://ip:port)") }, singleLine = true)
                OutlinedTextField(port, { port = it.filter(Char::isDigit).take(5) }, label = { Text("Port") }, singleLine = true,
                    keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Number))
                OutlinedTextField(pass, { pass = it }, label = { Text((if (type == "dav") "PIN" else "Password") + if (old != null) " (blank = keep)" else "") },
                    singleLine = true, visualTransformation = PasswordVisualTransformation(),
                    keyboardOptions = KeyboardOptions(keyboardType = if (type == "dav") KeyboardType.NumberPassword else KeyboardType.Password))
                OutlinedTextField(name, { name = it }, label = { Text("Name (optional)") }, singleLine = true)
                OutlinedTextField(user, { user = it }, label = { Text("User") }, singleLine = true)
                if (type == "dav") OutlinedTextField(path, { path = it }, label = { Text("Path") }, singleLine = true)
            }
        },
        confirmButton = {
            TextButton(enabled = ok, onClick = {
                val s = Server(old?.id ?: UUID.randomUUID().toString(), type, name.ifBlank { host }, host.trim(), port.toInt(),
                    user.trim(), if (type == "dav") path.trim().ifEmpty { "/dav/" } else "")
                state.prefs.saveServer(s)
                if (pass.isNotEmpty()) state.prefs.setSecret(s.id, pass)
                close()
            }) { Text("Save") }
        },
        dismissButton = { TextButton(onClick = close) { Text("Cancel") } },
    )
}
