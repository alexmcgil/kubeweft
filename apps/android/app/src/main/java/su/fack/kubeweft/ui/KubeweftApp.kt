package su.fack.kubeweft.ui

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.material3.darkColorScheme
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp

data class StatusCopy(val title: String, val status: String)

fun statusCopy() = StatusCopy(title = "Kubeweft", status = "No cluster connected")

data class NamespaceItem(val name: String, val kind: String, val detail: String)

fun previewNamespace() = listOf(
    NamespaceItem("Documents", "directory", "Cluster namespace"),
    NamespaceItem("Downloads", "directory", "Cluster namespace"),
    NamespaceItem("Projects", "directory", "Cluster namespace"),
    NamespaceItem("Photos", "directory", "Cluster namespace"),
    NamespaceItem("hello.txt", "file", "15 B · 2 replicas"),
)

@Composable
fun KubeweftApp() {
    val entries = remember { previewNamespace() }
    var selected by remember { mutableStateOf<NamespaceItem?>(null) }

    MaterialTheme(colorScheme = darkColorScheme()) {
        Surface(modifier = Modifier.fillMaxSize(), color = Color(0xFF0F172A)) {
            Column(modifier = Modifier.padding(20.dp)) {
                Text(
                    text = statusCopy().title,
                    style = MaterialTheme.typography.headlineMedium,
                    fontWeight = FontWeight.Bold,
                )
                Text(text = "Mobile client preview", color = Color(0xFF94A3B8))
                Spacer(modifier = Modifier.height(28.dp))
                Text(text = "/home/user", style = MaterialTheme.typography.titleMedium)
                Spacer(modifier = Modifier.height(10.dp))
                LazyColumn(verticalArrangement = Arrangement.spacedBy(8.dp)) {
                    items(entries) { entry ->
                        NamespaceRow(entry, selected == entry) { selected = entry }
                    }
                }
                Spacer(modifier = Modifier.height(24.dp))
                Text(
                    text = selected?.let { "Selected /home/user/${it.name}" }
                        ?: "Select an entry",
                    color = Color(0xFFCBD5E1),
                )
                Spacer(modifier = Modifier.weight(1f))
                Text(
                    text = "${statusCopy().status} · preview data",
                    color = Color(0xFF64748B),
                    style = MaterialTheme.typography.bodySmall,
                )
            }
        }
    }
}

@Composable
private fun NamespaceRow(entry: NamespaceItem, selected: Boolean, onClick: () -> Unit) {
    Row(
        modifier = Modifier
            .fillMaxWidth()
            .background(
                color = if (selected) Color(0xFF334155) else Color(0xFF1E293B),
                shape = RoundedCornerShape(10.dp),
            )
            .clickable(onClick = onClick)
            .padding(horizontal = 16.dp, vertical = 14.dp),
    ) {
        Text(
            text = if (entry.kind == "directory") "▸  ${entry.name}" else "   ${entry.name}",
            modifier = Modifier.weight(1f),
            color = Color(0xFFF8FAFC),
        )
        Text(
            text = entry.detail,
            color = Color(0xFF94A3B8),
            style = MaterialTheme.typography.bodySmall,
        )
    }
}
