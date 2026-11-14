package com.hafij.harborfiles

import androidx.compose.foundation.isSystemInDarkTheme
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.darkColorScheme
import androidx.compose.material3.lightColorScheme
import androidx.compose.runtime.Composable
import androidx.compose.ui.graphics.Color

val ACCENTS = listOf(
    "Harbor blue" to Color(0xFF0067C0),
    "Teal" to Color(0xFF00897B),
    "Violet" to Color(0xFF6B4FBB),
    "Rose" to Color(0xFFC2185B),
    "Orange" to Color(0xFFE65100),
    "Graphite" to Color(0xFF455A64),
)

/** Colour per file kind, for icons. */
fun kindColor(k: Kind): Color = when (k) {
    Kind.Folder -> Color(0xFFF4B400)
    Kind.Image -> Color(0xFF34A853)
    Kind.Video -> Color(0xFFE8453C)
    Kind.Audio -> Color(0xFF9C27B0)
    Kind.Doc -> Color(0xFF1E88E5)
    Kind.Archive -> Color(0xFF8D6E63)
    Kind.Apk -> Color(0xFF3DDC84)
    Kind.Other -> Color(0xFF78909C)
}

@Composable
fun HarborTheme(prefs: Prefs, content: @Composable () -> Unit) {
    val dark = when (prefs.theme) { 1 -> false; 2 -> true; else -> isSystemInDarkTheme() }
    val a = ACCENTS.getOrElse(prefs.accent) { ACCENTS[0] }.second
    val scheme = if (dark) darkColorScheme(
        primary = a.copy(red = (a.red + 0.25f).coerceAtMost(1f), green = (a.green + 0.25f).coerceAtMost(1f), blue = (a.blue + 0.25f).coerceAtMost(1f)),
        onPrimary = Color.White,
        primaryContainer = a.copy(alpha = 0.35f),
        background = Color(0xFF111214), surface = Color(0xFF1C1D21), surfaceVariant = Color(0xFF26272C),
        surfaceContainer = Color(0xFF1C1D21), surfaceContainerHigh = Color(0xFF26272C),
        onBackground = Color(0xFFE8E8EC), onSurface = Color(0xFFE8E8EC), onSurfaceVariant = Color(0xFF9EA0A8),
        outlineVariant = Color(0xFF34353B),
    ) else lightColorScheme(
        primary = a, onPrimary = Color.White, primaryContainer = a.copy(alpha = 0.14f),
        background = Color(0xFFF3F4F6), surface = Color.White, surfaceVariant = Color(0xFFECEDF1),
        surfaceContainer = Color.White, surfaceContainerHigh = Color(0xFFF7F7F9),
        onBackground = Color(0xFF16171A), onSurface = Color(0xFF16171A), onSurfaceVariant = Color(0xFF6B6E76),
        outlineVariant = Color(0xFFE2E3E8),
    )
    MaterialTheme(colorScheme = scheme, content = content)
}
