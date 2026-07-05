package com.itsanas.client.network

import java.util.concurrent.TimeUnit
import kotlinx.serialization.json.Json
import okhttp3.MediaType.Companion.toMediaType
import okhttp3.OkHttpClient
import com.jakewharton.retrofit2.converter.kotlinx.serialization.asConverterFactory
import retrofit2.Retrofit

/** Builds a [DaemonApi] for a given base URL — see [com.itsanas.client.SettingsScreen]. */
object RetrofitClient {
    private val json = Json { ignoreUnknownKeys = true }

    fun create(baseUrl: String): DaemonApi {
        val normalized = if (baseUrl.endsWith("/")) baseUrl else "$baseUrl/"
        val client =
            OkHttpClient.Builder()
                // Generous timeouts: file transfers can be large, and the
                // daemon's own body-size limit is disabled to match (see
                // itsanas-daemon's http.rs) — the client shouldn't be the
                // thing that gives up first.
                .connectTimeout(10, TimeUnit.SECONDS)
                .readTimeout(5, TimeUnit.MINUTES)
                .writeTimeout(5, TimeUnit.MINUTES)
                .build()

        return Retrofit.Builder()
            .baseUrl(normalized)
            .client(client)
            .addConverterFactory(json.asConverterFactory("application/json".toMediaType()))
            .build()
            .create(DaemonApi::class.java)
    }
}
