package com.hafij.harborfiles

import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.navigationBarsPadding
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.statusBarsPadding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.rounded.CameraAlt
import androidx.compose.material.icons.rounded.ContentPaste
import androidx.compose.material.icons.rounded.Delete
import androidx.compose.material.icons.rounded.Download
import androidx.compose.material.icons.rounded.Folder
import androidx.compose.material.icons.rounded.GridView
import androidx.compose.material.icons.rounded.History
import androidx.compose.material.icons.rounded.InsertDriveFile
import androidx.compose.material.icons.rounded.Lan
import androidx.compose.material.icons.rounded.PhoneAndroid
import androidx.compose.material.icons.rounded.Screenshot
import androidx.compose.material.icons.rounded.SdStorage
import androidx.compose.material.icons.rounded.Search
import androidx.compose.material.icons.rounded.Settings
import androidx.compose.material.icons.rounded.Star
import androidx.compose.material3.FilledTonalButton
import androidx.compose.material3.Icon
import androidx.compose.material3.LinearProgressIndicator
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.produceState
import androidx.compose.runtime.remember
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
import java.io.File

/**
 * Tablet sidebar (landscape, 840 dp and wider): main tabs, places, favourites,
 * storage use, the paste clipboard and Share with PC, always one tap away.
 */
@Composable
fun Sidebar(state: AppState) {
    val c = MaterialTheme.colorScheme
    val top = state.stack.last()
    val rail = state.stack.lastOrNull { it == Screen.Search || it == Screen.Favorites || it == Screen.Trash || it == Screen.Settings }
    val openPath = (top as? Screen.Folder)?.path
    fun home(tab: Int) { while (state.stack.size > 1) state.pop(); state.tab = tab }
    fun open(s: Screen) { while (state.stack.size > 1) state.pop(); state.push(s) }
    fun place(path: String, title: String) {
        while (state.stack.size > 1) state.pop()
        state.openFolder(state.local, path, path, title)
    }
    val vols by produceState<List<Volume>>(emptyList(), state.rev) { value = withContext(Dispatchers.IO) { volumes(state.ctx) } }
    val sources = remember(state.rev) { sourceDirs() }
    val favs = remember(state.prefs.favorites, state.rev) {
        state.prefs.favorites.map(::File).filter { it.exists() }.sortedBy { it.name.lowercase() }
    }
    val info by ServerService.state

    Surface(Modifier.width(280.dp).fillMaxHeight(), color = c.surface) {
        Column(
            Modifier.fillMaxHeight().statusBarsPadding().navigationBarsPadding()
                .verticalScroll(rememberScrollState()).padding(horizontal = 12.dp, vertical = 12.dp),
        ) {
            Text("Harbor Files", Modifier.padding(start = 12.dp, top = 4.dp, bottom = 12.dp),
                style = MaterialTheme.typography.titleLarge, fontWeight = FontWeight.SemiBold)
            Row(
                Modifier.fillMaxWidth().clip(CircleShape).background(c.surfaceVariant.copy(alpha = 0.6f))
                    .clickable { open(Screen.Search) }.padding(horizontal = 16.dp, vertical = 12.dp),
                verticalAlignment = Alignment.CenterVertically,
            ) {
                Icon(Icons.Rounded.Search, null, Modifier.size(20.dp), tint = c.onSurfaceVariant)
                Spacer(Modifier.width(12.dp))
                Text("Search files", color = c.onSurfaceVariant)
            }
            Spacer(Modifier.height(10.dp))
            SideItem("Recent", Icons.Rounded.History, rail == null && openPath == null && state.tab == 0) { home(0) }
            SideItem("Browse", Icons.Rounded.GridView, rail == null && openPath == null && state.tab == 1) { home(1) }
            SideItem("Connect", Icons.Rounded.Lan, rail == null && openPath == null && state.tab == 2,
                badge = if (info != null) "On" else null) { home(2) }

            state.clip?.let { clip ->
                Spacer(Modifier.height(10.dp))
                Surface(shape = RoundedCornerShape(16.dp), color = c.primaryContainer) {
                    Row(Modifier.fillMaxWidth().padding(start = 14.dp, end = 4.dp, top = 6.dp, bottom = 6.dp),
                        verticalAlignment = Alignment.CenterVertically) {
                        Icon(Icons.Rounded.ContentPaste, null, Modifier.size(20.dp), tint = c.onPrimaryContainer)
                        Spacer(Modifier.width(10.dp))
                        Text("${clip.items.size} to ${if (clip.move) "move" else "copy"}", Modifier.weight(1f),
                            color = c.onPrimaryContainer, style = MaterialTheme.typography.bodyMedium)
                        TextButton(onClick = { state.clip = null }) { Text("Clear") }
                    }
                }
            }

            SideHeader("Places")
            SideItem("Internal storage", Icons.Rounded.PhoneAndroid, openPath == PHONE_ROOT) { place(PHONE_ROOT, "Internal storage") }
            val dl = File(PHONE_ROOT, "Download").path
            SideItem("Downloads", Icons.Rounded.Download, openPath == dl) { place(dl, "Downloads") }
            sources.forEach { (n, f) ->
                val ic = when (n) { "Camera" -> Icons.Rounded.CameraAlt; "Screenshots" -> Icons.Rounded.Screenshot; else -> Icons.Rounded.Folder }
                SideItem(n, ic, openPath == f.path) { place(f.path, n) }
            }
            vols.filter { it.path != PHONE_ROOT }.forEach { v ->
                SideItem(v.title, Icons.Rounded.SdStorage, openPath == v.path) { place(v.path, v.title) }
            }

            if (favs.isNotEmpty()) {
                SideHeader("Favourites")
                favs.take(8).forEach { f ->
                    SideItem(f.name, if (f.isDirectory) Icons.Rounded.Star else Icons.Rounded.InsertDriveFile, openPath == f.path) {
                        if (f.isDirectory) place(f.path, f.name)
                        else state.scope.launch { openEntry(state, state.local, Entry(f.name, f.path, false, f.length(), f.lastModified())) }
                    }
                }
                if (favs.size > 8) SideItem("All favourites (${favs.size})", Icons.Rounded.Star, rail == Screen.Favorites) { open(Screen.Favorites) }
            }

            SideHeader("Share with PC")
            Surface(shape = RoundedCornerShape(16.dp), color = c.surfaceVariant.copy(alpha = 0.5f)) {
                Column(Modifier.fillMaxWidth().padding(14.dp)) {
                    val i = info
                    if (i == null) {
                        Text("Off", fontWeight = FontWeight.SemiBold)
                        Text("Let your PC open this tablet over Wi-Fi.", style = MaterialTheme.typography.bodySmall, color = c.onSurfaceVariant)
                        Spacer(Modifier.height(8.dp))
                        FilledTonalButton(onClick = { ServerService.error.value = null; ServerService.start(state.ctx) }) { Text("Start sharing") }
                    } else {
                        Text("Sharing on Wi-Fi", fontWeight = FontWeight.SemiBold, color = c.primary)
                        Text(i.url, fontFamily = FontFamily.Monospace, style = MaterialTheme.typography.bodySmall)
                        Text("PIN ${i.pin.chunked(3).joinToString(" ")}", fontFamily = FontFamily.Monospace, fontWeight = FontWeight.SemiBold)
                        Row {
                            TextButton(onClick = { home(2) }) { Text("Show QR") }
                            TextButton(onClick = { ServerService.stop(state.ctx) }) { Text("Stop") }
                        }
                    }
                }
            }

            if (vols.isNotEmpty()) {
                SideHeader("Storage")
                vols.forEach { v ->
                    Column(Modifier.fillMaxWidth().clip(RoundedCornerShape(12.dp))
                        .clickable { place(v.path, v.title) }.padding(horizontal = 12.dp, vertical = 6.dp)) {
                        Text(v.title, style = MaterialTheme.typography.bodyMedium)
                        Spacer(Modifier.height(4.dp))
                        LinearProgressIndicator(
                            progress = { if (v.total > 0) 1f - v.free.toFloat() / v.total else 0f },
                            Modifier.fillMaxWidth().height(6.dp).clip(CircleShape),
                        )
                        Spacer(Modifier.height(2.dp))
                        Text("${fmtSize(v.free)} free of ${fmtSize(v.total)}", style = MaterialTheme.typography.bodySmall, color = c.onSurfaceVariant)
                    }
                }
            }

            Spacer(Modifier.height(12.dp))
            SideItem("Recently deleted", Icons.Rounded.Delete, rail == Screen.Trash) { open(Screen.Trash) }
            SideItem("Settings", Icons.Rounded.Settings, rail == Screen.Settings) { open(Screen.Settings) }
        }
    }
}

@Composable
private fun SideHeader(text: String) {
    Text(text, Modifier.padding(start = 12.dp, top = 18.dp, bottom = 4.dp),
        style = MaterialTheme.typography.labelLarge, color = MaterialTheme.colorScheme.onSurfaceVariant)
}

@Composable
private fun SideItem(label: String, icon: ImageVector, selected: Boolean, badge: String? = null, onClick: () -> Unit) {
    val c = MaterialTheme.colorScheme
    Row(
        Modifier.fillMaxWidth().clip(CircleShape).background(if (selected) c.primaryContainer else Color.Transparent)
            .clickable(onClick = onClick).padding(horizontal = 14.dp, vertical = 10.dp),
        verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(14.dp),
    ) {
        Icon(icon, null, Modifier.size(22.dp), tint = if (selected) c.primary else c.onSurfaceVariant)
        Text(label, Modifier.weight(1f), maxLines = 1, overflow = TextOverflow.Ellipsis,
            fontWeight = if (selected) FontWeight.SemiBold else FontWeight.Normal,
            color = if (selected) c.primary else c.onSurface)
        badge?.let {
            Box(Modifier.clip(CircleShape).background(c.primary).padding(horizontal = 8.dp, vertical = 2.dp)) {
                Text(it, style = MaterialTheme.typography.labelSmall, color = c.onPrimary)
            }
        }
    }
}
