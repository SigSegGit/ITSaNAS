package com.itsanas.client

import android.provider.OpenableColumns
import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.result.contract.ActivityResultContracts
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.material3.Button
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.IconButton
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.unit.dp
import com.itsanas.client.network.FileInfo

@Composable
fun AppRoot(viewModel: DaemonViewModel) {
    val state by viewModel.state.collectAsState()
    val status = state.status

    when {
        state.baseUrl.isBlank() ->
            SettingsScreen(initialUrl = "") { url -> viewModel.setBaseUrl(url) }

        status == null ->
            ConnectingScreen(error = state.error) { viewModel.setBaseUrl("") }

        !status.hasAccount ->
            SetupScreen(busy = state.busy, error = state.error) { password -> viewModel.setup(password) }

        !status.unlocked ->
            UnlockScreen(busy = state.busy, error = state.error) { password -> viewModel.unlock(password) }

        else ->
            MainScreen(state = state, viewModel = viewModel) { viewModel.setBaseUrl("") }
    }
}

@Composable
fun SettingsScreen(initialUrl: String, onSave: (String) -> Unit) {
    var url by remember { mutableStateOf(initialUrl.ifBlank { "http://" }) }
    Column(modifier = Modifier.fillMaxSize().padding(24.dp)) {
        Text("Connect to your itsanas-daemon")
        Spacer(modifier = Modifier.height(8.dp))
        Text(
            "This isn't the daemon itself — it's a thin client that talks " +
                "to one running elsewhere (your desktop, or a NAS box), " +
                "over your LAN or a tunnel/VPN you've already set up.",
        )
        Spacer(modifier = Modifier.height(16.dp))
        OutlinedTextField(
            value = url,
            onValueChange = { url = it },
            label = { Text("Daemon address, e.g. http://192.168.1.20:4279") },
            modifier = Modifier.fillMaxWidth(),
        )
        Spacer(modifier = Modifier.height(16.dp))
        Button(onClick = { onSave(url.trim()) }) { Text("Connect") }
    }
}

@Composable
fun ConnectingScreen(error: String?, onChangeServer: () -> Unit) {
    Column(modifier = Modifier.fillMaxSize().padding(24.dp)) {
        Text("Connecting...")
        error?.let {
            Spacer(modifier = Modifier.height(8.dp))
            Text(it)
        }
        Spacer(modifier = Modifier.height(16.dp))
        TextButton(onClick = onChangeServer) { Text("Change server address") }
    }
}

@Composable
fun SetupScreen(busy: Boolean, error: String?, onCreate: (String) -> Unit) {
    var password by remember { mutableStateOf("") }
    var confirm by remember { mutableStateOf("") }
    Column(modifier = Modifier.fillMaxSize().padding(24.dp)) {
        Text(
            "Create your account password. This encrypts everything in " +
                "your vault — there is no recovery if you lose it.",
        )
        Spacer(modifier = Modifier.height(16.dp))
        OutlinedTextField(
            value = password,
            onValueChange = { password = it },
            label = { Text("Password") },
            modifier = Modifier.fillMaxWidth(),
        )
        OutlinedTextField(
            value = confirm,
            onValueChange = { confirm = it },
            label = { Text("Confirm password") },
            modifier = Modifier.fillMaxWidth(),
        )
        Spacer(modifier = Modifier.height(16.dp))
        Button(enabled = !busy && password.isNotEmpty() && password == confirm, onClick = { onCreate(password) }) {
            Text("Create account")
        }
        error?.let {
            Spacer(modifier = Modifier.height(8.dp))
            Text(it)
        }
    }
}

@Composable
fun UnlockScreen(busy: Boolean, error: String?, onUnlock: (String) -> Unit) {
    var password by remember { mutableStateOf("") }
    Column(modifier = Modifier.fillMaxSize().padding(24.dp)) {
        Text("Unlock your vault")
        Spacer(modifier = Modifier.height(16.dp))
        OutlinedTextField(
            value = password,
            onValueChange = { password = it },
            label = { Text("Password") },
            modifier = Modifier.fillMaxWidth(),
        )
        Spacer(modifier = Modifier.height(16.dp))
        Button(enabled = !busy && password.isNotEmpty(), onClick = { onUnlock(password) }) { Text("Unlock") }
        error?.let {
            Spacer(modifier = Modifier.height(8.dp))
            Text(it)
        }
    }
}

@Composable
fun MainScreen(state: UiState, viewModel: DaemonViewModel, onChangeServer: () -> Unit) {
    val context = LocalContext.current
    var pendingDownloadName by remember { mutableStateOf<String?>(null) }

    val pickUploadLauncher =
        rememberLauncherForActivityResult(ActivityResultContracts.OpenDocument()) { uri ->
            if (uri == null) return@rememberLauncherForActivityResult
            val name =
                context.contentResolver.query(uri, null, null, null, null)?.use { cursor ->
                    val nameIndex = cursor.getColumnIndex(OpenableColumns.DISPLAY_NAME)
                    if (nameIndex >= 0 && cursor.moveToFirst()) cursor.getString(nameIndex) else null
                } ?: uri.lastPathSegment ?: "upload.bin"
            viewModel.uploadFile(name, uri)
        }

    val createDownloadLauncher =
        rememberLauncherForActivityResult(ActivityResultContracts.CreateDocument("application/octet-stream")) { uri ->
            val name = pendingDownloadName
            pendingDownloadName = null
            if (uri != null && name != null) viewModel.downloadFile(name, uri)
        }

    Column(modifier = Modifier.fillMaxSize().padding(24.dp)) {
        Text("Unlocked")
        Spacer(modifier = Modifier.height(4.dp))
        Text("Synced folder on the daemon's machine: ${state.status?.syncedFolder}")
        Spacer(modifier = Modifier.height(12.dp))

        Row {
            Button(onClick = { pickUploadLauncher.launch(arrayOf("*/*")) }) { Text("Upload") }
            TextButton(onClick = { viewModel.lock() }) { Text("Lock") }
            TextButton(onClick = onChangeServer) { Text("Change server") }
        }

        if (state.busy) {
            Spacer(modifier = Modifier.height(8.dp))
            CircularProgressIndicator()
        }
        state.error?.let {
            Spacer(modifier = Modifier.height(8.dp))
            Text(it)
        }

        state.status?.vaultHealth?.unhealthyFiles?.takeIf { it.isNotEmpty() }?.let { unhealthy ->
            Spacer(modifier = Modifier.height(8.dp))
            Text(
                "${unhealthy.size} file(s) need attention (failed a background " +
                    "integrity check): ${unhealthy.joinToString(", ")}",
            )
        }

        Spacer(modifier = Modifier.height(16.dp))
        Text("${state.files.size} file(s) in the vault:")
        LazyColumn {
            items(state.files) { file ->
                FileRow(
                    file = file,
                    onDownload = {
                        pendingDownloadName = file.name
                        createDownloadLauncher.launch(file.name)
                    },
                    onDelete = { viewModel.deleteFile(file.name) },
                )
                HorizontalDivider()
            }
        }
    }
}

@Composable
private fun FileRow(file: FileInfo, onDownload: () -> Unit, onDelete: () -> Unit) {
    Row(
        modifier = Modifier.fillMaxWidth().padding(vertical = 8.dp),
        horizontalArrangement = Arrangement.SpaceBetween,
    ) {
        Text("${file.name} (${file.size} bytes)")
        Row {
            IconButton(onClick = onDownload) { Text("↓") }
            IconButton(onClick = onDelete) { Text("✕") }
        }
    }
}
