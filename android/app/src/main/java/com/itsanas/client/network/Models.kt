package com.itsanas.client.network

import kotlinx.serialization.SerialName
import kotlinx.serialization.Serializable

@Serializable
data class StatusResponse(
    @SerialName("has_account") val hasAccount: Boolean,
    val unlocked: Boolean,
    @SerialName("synced_folder") val syncedFolder: String,
    // Always null while locked or before the first background scrub (D7)
    // has completed - a locked vault reveals nothing, not even this.
    @SerialName("vault_health") val vaultHealth: VaultHealth? = null,
)

@Serializable
data class VaultHealth(
    @SerialName("healthy_shards") val healthyShards: Long,
    @SerialName("unhealthy_files") val unhealthyFiles: List<String>,
)

@Serializable
data class FileInfo(
    val name: String,
    val size: Long,
)

@Serializable
data class PasswordRequest(
    val password: String,
)
