package dev.kubeweft.android.ui

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier

data class StatusCopy(
    val title: String,
    val status: String,
)

fun statusCopy(): StatusCopy = StatusCopy(
    title = "Kubeweft",
    status = "No cluster connected",
)

@Composable
fun KubeweftApp() {
    val copy = statusCopy()

    MaterialTheme {
        Surface(modifier = Modifier.fillMaxSize()) {
            Column(
                modifier = Modifier.fillMaxSize(),
                verticalArrangement = Arrangement.Center,
                horizontalAlignment = Alignment.CenterHorizontally,
            ) {
                Text(text = copy.title, style = MaterialTheme.typography.headlineMedium)
                Text(text = copy.status, style = MaterialTheme.typography.bodyLarge)
            }
        }
    }
}
