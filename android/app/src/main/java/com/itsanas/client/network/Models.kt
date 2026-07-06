package com.itsanas.client.network

import kotlinx.serialization.SerialName
import kotlinx.serialization.Serializable

@Serializable
data class StatusResponse(
    @SerialName("has_account") val hasAccount: Boolean,
    val unlocked: Boolean,
    @SerialName("synced_folder") val syncedFolder: String,
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
