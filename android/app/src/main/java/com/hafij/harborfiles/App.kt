package com.hafij.harborfiles

import android.Manifest
import android.content.Context
import android.content.Intent
import android.net.Uri
import android.os.Build
import android.os.Bundle
import android.os.Environment
import android.provider.Settings
import androidx.activity.ComponentActivity
import androidx.activity.compose.BackHandler
import androidx.activity.compose.setContent
import androidx.activity.enableEdgeToEdge
import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.statusBarsPadding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.rounded.Delete
import androidx.compose.material.icons.rounded.Settings
import androidx.compose.material.icons.rounded.Star
import androidx.compose.material3.HorizontalDivider
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.ui.platform.LocalConfiguration
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.navigationBarsPadding
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.rounded.Folder
import androidx.compose.material.icons.rounded.GridView
import androidx.compose.material.icons.rounded.History
import androidx.compose.material.icons.rounded.Lan
import androidx.compose.material.icons.rounded.Search
import androidx.compose.material3.Button
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.SnackbarHost
import androidx.compose.material3.SnackbarHostState
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.mutableStateListOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.dp
import androidx.compose.ui.window.Dialog
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
import java.io.File

data class Clip(val fs: Fs, val items: List<Entry>, val move: Boolean)

sealed interface Screen {
    data object Home : Screen
    data class Folder(val fs: Fs, val path: String, val base: String, val baseTitle: String) : Screen
    data class Category(val cat: Cat) : Screen
    data class Group(val title: String, val items: List<Entry>) : Screen
    data object Favorites : Screen
    data object Trash : Screen
    data object Settings : Screen
    data object Search : Screen
}

class AppState(val ctx: Context, val prefs: Prefs, val scope: CoroutineScope) {
    val local = LocalFs({ prefs.showHidden }, { prefs.hideSystem })
    val stack = mutableStateListOf<Screen>(Screen.Home)
    var tab by mutableIntStateOf(0)
    var clip by mutableStateOf<Clip?>(null)
    var busy by mutableStateOf<String?>(null)
    var rev by mutableIntStateOf(0)
    val snack = SnackbarHostState()
    val trash get() = Trash(File(PHONE_ROOT, TRASH_DIR))

    fun push(s: Screen) { stack.add(s) }
    fun pop() { if (stack.size > 1) stack.removeAt(stack.size - 1) }

    fun openFolder(fs: Fs, path: String, base: String = fs.root, baseTitle: String = fs.title) =
        push(Screen.Folder(fs, path, base, baseTitle))

    fun toast(msg: String) { scope.launch { snack.showSnackbar(msg) } }

    /** "Stream or download?" for a big video or music file on a server. */
    class StreamAsk(val name: String, val size: Long, val answer: (String?, Boolean) -> Unit)
    var streamAsk by mutableStateOf<StreamAsk?>(null)

    /** "stream", "download" or null (cancelled). "Remember" saves it in Settings. */
    suspend fun askStream(e: Entry): String? {
        val d = kotlinx.coroutines.CompletableDeferred<String?>()
        streamAsk = StreamAsk(e.name, e.size) { ans, remember ->
            streamAsk = null
            if (remember && ans != null) prefs.remoteMedia = if (ans == "stream") 1 else 2
            d.complete(ans)
        }
        return d.await()
    }

    /** Runs blocking work off the main thread with a busy dialog, then refreshes lists. */
    fun run(label: String, work: suspend () -> Unit) {
        scope.launch {
            busy = label
            try { withContext(Dispatchers.IO) { work() } } catch (e: Exception) {
                toast(e.message ?: "Something went wrong")
            } finally { busy = null; rev++ }
        }
    }
}

class MainActivity : ComponentActivity() {
    private var resumes by mutableIntStateOf(0)

    private var askTab by mutableIntStateOf(-1)

    override fun onResume() { super.onResume(); resumes++; PlayTemp.cleanup() }

    override fun onNewIntent(intent: android.content.Intent) {
        super.onNewIntent(intent)
        askTab = intent.getIntExtra(EXTRA_TAB, -1)
    }

    companion object { const val EXTRA_TAB = "tab" }

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        enableEdgeToEdge()
        val prefs = Prefs(this)
        if (savedInstanceState == null) PlayTemp.cleanAll(this)     // played copies left from last time
        askTab = intent.getIntExtra(EXTRA_TAB, -1)
        if (Build.VERSION.SDK_INT >= 33) requestPermissions(arrayOf(Manifest.permission.POST_NOTIFICATIONS), 1)
        setContent {
            HarborTheme(prefs) {
                val scope = rememberCoroutineScope()
                val state = remember { AppState(applicationContext, prefs, scope) }
                var allowed by remember { mutableStateOf(Environment.isExternalStorageManager()) }
                LaunchedEffect(resumes) {
                    allowed = Environment.isExternalStorageManager()
                    if (allowed && resumes > 0) state.rev++
                }
                LaunchedEffect(askTab) {
                    if (askTab >= 0) {
                        while (state.stack.size > 1) state.pop()
                        state.tab = askTab
                        askTab = -1
                    }
                }
                Surface(Modifier.fillMaxSize(), color = MaterialTheme.colorScheme.background) {
                    if (!allowed) PermissionGate() else Root(state)
                }
            }
        }
    }

    @Composable
    private fun PermissionGate() {
        Column(
            Modifier.fillMaxSize().padding(32.dp),
            verticalArrangement = Arrangement.Center, horizontalAlignment = Alignment.CenterHorizontally,
        ) {
            Icon(Icons.Rounded.Folder, null, Modifier.size(72.dp), tint = MaterialTheme.colorScheme.primary)
            Spacer(Modifier.height(16.dp))
            Text("Harbor Files needs access to all files", style = MaterialTheme.typography.titleLarge, textAlign = TextAlign.Center)
            Spacer(Modifier.height(8.dp))
            Text(
                "This lets it list, copy and share any kind of file on your phone. Nothing leaves the phone unless you send it.",
                textAlign = TextAlign.Center, color = MaterialTheme.colorScheme.onSurfaceVariant,
            )
            Spacer(Modifier.height(24.dp))
            Button(onClick = {
                startActivity(
                    Intent(Settings.ACTION_MANAGE_APP_ALL_FILES_ACCESS_PERMISSION, Uri.parse("package:$packageName"))
                )
            }) { Text("Allow access") }
        }
    }
}

@Composable
fun Root(state: AppState) {
    LaunchedEffect(Unit) {
        withContext(Dispatchers.IO) { runCatching { state.trash.expire(state.prefs.trashDays) } }
    }
    BackHandler(enabled = state.stack.size > 1) { state.pop() }
    BackHandler(enabled = state.stack.size == 1 && state.tab != 0) { state.tab = 0 }
    val width = LocalConfiguration.current.screenWidthDp
    val wide = width >= 600
    val side = width >= 840
    // Landscape tablets get the sidebar. Portrait tablets keep the bottom pill
    // (easy to reach) and One UI style tall headers that start lower down.
    CompositionLocalProvider(LocalWide provides wide, LocalSidebar provides side, LocalTall provides (wide && !side)) {
    Row(Modifier.fillMaxSize()) {
    if (side) Sidebar(state)
    Box(Modifier.weight(1f).fillMaxHeight()) {
        when (val s = state.stack.last()) {
            Screen.Home -> Home(state)
            is Screen.Folder -> FolderScreen(state, s)
            is Screen.Category -> CategoryScreen(state, s.cat)
            is Screen.Group -> GroupScreen(state, s)
            Screen.Favorites -> FavoritesScreen(state)
            Screen.Trash -> TrashScreen(state)
            Screen.Settings -> SettingsScreen(state)
            Screen.Search -> SearchScreen(state)
        }
        SnackbarHost(state.snack, Modifier.align(Alignment.BottomCenter).navigationBarsPadding().padding(bottom = if (side) 16.dp else 80.dp))
    }
    } }
    state.busy?.let { BusyDialog(it) }
    state.streamAsk?.let { StreamAskDialog(it) }
}

@Composable
private fun StreamAskDialog(a: AppState.StreamAsk) {
    var remember by remember(a) { mutableStateOf(false) }
    androidx.compose.material3.AlertDialog(
        onDismissRequest = { a.answer(null, false) },
        title = { Text("Play ${a.name}") },
        text = {
            Column {
                Text("This file is ${fmtSize(a.size)} and it's on the PC.")
                Text("Stream: plays at once and you can skip around. Stay on the same Wi-Fi while it plays.",
                    Modifier.padding(top = 8.dp))
                Text("Download: copies it to the phone first, then plays it. The copy is deleted when you come back.", Modifier.padding(top = 4.dp))
                Row(Modifier.padding(top = 8.dp).clickable { remember = !remember }, verticalAlignment = Alignment.CenterVertically) {
                    androidx.compose.material3.Checkbox(remember, { remember = it })
                    Text("Remember my choice")
                }
            }
        },
        confirmButton = { androidx.compose.material3.TextButton(onClick = { a.answer("stream", remember) }) { Text("Stream") } },
        dismissButton = { androidx.compose.material3.TextButton(onClick = { a.answer("download", remember) }) { Text("Download") } },
    )
}

@Composable
fun BusyDialog(label: String) {
    Dialog(onDismissRequest = {}) {
        Surface(shape = RoundedCornerShape(24.dp), color = MaterialTheme.colorScheme.surface) {
            Row(Modifier.padding(24.dp), verticalAlignment = Alignment.CenterVertically) {
                CircularProgressIndicator(Modifier.size(28.dp))
                Spacer(Modifier.width(16.dp))
                Text(label, maxLines = 2)
            }
        }
    }
}

@Composable
fun Home(state: AppState) {
    Box(Modifier.fillMaxSize()) {
        when (state.tab) {
            0 -> RecentScreen(state)
            1 -> BrowseScreen(state)
            else -> ConnectScreen(state)
        }
        if (!LocalSidebar.current) BottomPill(state, Modifier.align(Alignment.BottomCenter))
    }
}

@Composable
private fun BottomPill(state: AppState, modifier: Modifier) {
    Row(
        modifier.navigationBarsPadding().padding(bottom = 14.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Surface(shape = CircleShape, shadowElevation = 8.dp, color = MaterialTheme.colorScheme.surface) {
            Row(Modifier.padding(6.dp)) {
                PillItem("Recent", Icons.Rounded.History, state.tab == 0) { state.tab = 0 }
                PillItem("Browse", Icons.Rounded.GridView, state.tab == 1) { state.tab = 1 }
                PillItem("Connect", Icons.Rounded.Lan, state.tab == 2) { state.tab = 2 }
            }
        }
        Spacer(Modifier.width(12.dp))
        Surface(
            shape = CircleShape, shadowElevation = 8.dp, color = MaterialTheme.colorScheme.surface,
            modifier = Modifier.size(56.dp).clip(CircleShape).clickable { state.push(Screen.Search) },
        ) {
            Box(contentAlignment = Alignment.Center) { Icon(Icons.Rounded.Search, "Search") }
        }
    }
}

@Composable
private fun PillItem(label: String, icon: ImageVector, selected: Boolean, onClick: () -> Unit) {
    val c = MaterialTheme.colorScheme
    Column(
        Modifier.clip(CircleShape)
            .background(if (selected) c.primaryContainer else c.surface)
            .clickable(onClick = onClick)
            .padding(horizontal = 18.dp, vertical = 6.dp),
        horizontalAlignment = Alignment.CenterHorizontally,
    ) {
        Icon(icon, null, Modifier.size(22.dp), tint = if (selected) c.primary else c.onSurfaceVariant)
        Text(
            label, style = MaterialTheme.typography.labelSmall,
            fontWeight = if (selected) FontWeight.SemiBold else FontWeight.Normal,
            color = if (selected) c.primary else c.onSurfaceVariant,
        )
    }
}

/** Big page title like the Mi file manager. */
@Composable
fun BigTitle(text: String, modifier: Modifier = Modifier, trailing: @Composable () -> Unit = {}) {
    if (LocalTall.current) { TallTitle(text, modifier, trailing); return }
    Row(modifier.fillMaxWidth().padding(start = 20.dp, end = 8.dp, top = 12.dp, bottom = 8.dp), verticalAlignment = Alignment.CenterVertically) {
        Text(text, style = MaterialTheme.typography.headlineLarge, fontWeight = FontWeight.SemiBold, modifier = Modifier.weight(1f))
        trailing()
    }
}

/**
 * One UI style header for portrait tablets: the title sits low in an empty top
 * area, so the first rows land within thumb reach. It scrolls away with the page.
 */
@Composable
fun TallTitle(text: String, modifier: Modifier = Modifier, trailing: @Composable () -> Unit = {}) {
    val h = LocalConfiguration.current.screenHeightDp * 0.26f
    Row(
        modifier.fillMaxWidth().height(h.dp).padding(start = 28.dp, end = 12.dp, bottom = 20.dp),
        verticalAlignment = Alignment.Bottom,
    ) {
        Text(text, style = MaterialTheme.typography.displaySmall, fontWeight = FontWeight.SemiBold,
            maxLines = 2, modifier = Modifier.weight(1f))
        trailing()
    }
}
