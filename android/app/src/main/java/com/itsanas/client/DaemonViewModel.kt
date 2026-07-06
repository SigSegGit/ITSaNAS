package com.itsanas.client

import android.app.Application
import android.content.Context
import android.net.Uri
import androidx.lifecycle.AndroidViewModel
import androidx.lifecycle.viewModelScope
import com.itsanas.client.network.DaemonApi
import com.itsanas.client.network.FileInfo
import com.itsanas.client.network.PasswordRequest
import com.itsanas.client.network.RetrofitClient
import com.itsanas.client.network.StatusResponse
import com.itsanas.client.network.uriRequestBody
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.update
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext

private const val PREFS_NAME = "itsanas"
private const val KEY_BASE_URL = "base_url"

data class UiState(
    val baseUrl: String = "",
    val status: StatusResponse? = null,
    val files: List<FileInfo> = emptyList(),
    val error: String? = null,
    val busy: Boolean = false,
)

/**
 * Holds the connection to itsanas-daemon and everything the UI needs.
 * Unlike itsanas-gui, this app never runs the daemon itself — it's D9's
 * "thin client", reaching a daemon running elsewhere (another machine on
 * the LAN, or over a tunnel) at a base URL the user configures once.
 */
class DaemonViewModel(application: Application) : AndroidViewModel(application) {
    private val prefs = application.getSharedPreferences(PREFS_NAME, Context.MODE_PRIVATE)
    private var api: DaemonApi? = null

    private val _state = MutableStateFlow(UiState(baseUrl = prefs.getString(KEY_BASE_URL, "") ?: ""))
    val state: StateFlow<UiState> = _state.asStateFlow()

    init {
        if (_state.value.baseUrl.isNotBlank()) {
            api = RetrofitClient.create(_state.value.baseUrl)
            refreshStatus()
        }
    }

    fun setBaseUrl(url: String) {
        prefs.edit().putString(KEY_BASE_URL, url).apply()
        api = RetrofitClient.create(url)
        _state.update { UiState(baseUrl = url) }
        refreshStatus()
    }

    fun refreshStatus() {
        val api = api ?: return
        viewModelScope.launch {
            try {
                val status = api.status()
                _state.update { it.copy(status = status, error = null) }
                if (status.hasAccount && status.unlocked) refreshFiles()
            } catch (e: Exception) {
                _state.update { it.copy(error = "Can't reach the daemon: ${e.message}") }
            }
        }
    }

    fun setup(password: String) =
        runAction { api -> api.setup(PasswordRequest(password)) }

    fun unlock(password: String) =
        runAction { api -> api.unlock(PasswordRequest(password)) }

    fun lock() =
        runAction { api -> api.lock() }

    fun refreshFiles() {
        val api = api ?: return
        viewModelScope.launch {
            try {
                val files = api.listFiles()
                _state.update { it.copy(files = files, error = null) }
            } catch (e: Exception) {
                _state.update { it.copy(error = "Couldn't list files: ${e.message}") }
            }
        }
    }

    fun uploadFile(name: String, uri: Uri) {
        val api = api ?: return
        val context = getApplication<Application>()
        viewModelScope.launch {
            _state.update { it.copy(busy = true) }
            try {
                api.putFile(name, uriRequestBody(context, uri))
                refreshFiles()
            } catch (e: Exception) {
                _state.update { it.copy(error = "Upload of $name failed: ${e.message}") }
            } finally {
                _state.update { it.copy(busy = false) }
            }
        }
    }

    fun downloadFile(name: String, destUri: Uri) {
        val api = api ?: return
        val context = getApplication<Application>()
        viewModelScope.launch {
            _state.update { it.copy(busy = true) }
            try {
                val body = api.getFile(name)
                withContext(Dispatchers.IO) {
                    val output =
                        context.contentResolver.openOutputStream(destUri)
                            ?: error("couldn't open $destUri")
                    output.use { out -> body.byteStream().use { it.copyTo(out) } }
                }
            } catch (e: Exception) {
                _state.update { it.copy(error = "Download of $name failed: ${e.message}") }
            } finally {
                _state.update { it.copy(busy = false) }
            }
        }
    }

    fun deleteFile(name: String) {
        val api = api ?: return
        viewModelScope.launch {
            _state.update { it.copy(busy = true) }
            try {
                api.deleteFile(name)
                _state.update { it.copy(error = null) }
                refreshFiles()
            } catch (e: Exception) {
                _state.update { it.copy(error = "Couldn't delete $name: ${e.message}") }
            } finally {
                _state.update { it.copy(busy = false) }
            }
        }
    }

    private fun runAction(block: suspend (DaemonApi) -> Unit) {
        val api = api ?: return
        viewModelScope.launch {
            _state.update { it.copy(busy = true) }
            try {
                block(api)
                _state.update { it.copy(error = null) }
                refreshStatus()
            } catch (e: Exception) {
                _state.update { it.copy(error = e.message ?: "Request failed") }
            } finally {
                _state.update { it.copy(busy = false) }
            }
        }
    }
}
