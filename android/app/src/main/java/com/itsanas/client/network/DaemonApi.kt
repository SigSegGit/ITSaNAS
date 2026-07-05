package com.itsanas.client.network

import okhttp3.RequestBody
import okhttp3.ResponseBody
import retrofit2.Response
import retrofit2.http.Body
import retrofit2.http.DELETE
import retrofit2.http.GET
import retrofit2.http.POST
import retrofit2.http.PUT
import retrofit2.http.Path
import retrofit2.http.Streaming

/**
 * Thin client over itsanas-daemon's local HTTP API (D9) — the same API
 * itsanas-gui talks to on the desktop, reached here over whatever
 * tunnel/VPN gets the phone to the machine actually running the daemon
 * (the daemon itself only ever binds to 127.0.0.1, by design).
 */
interface DaemonApi {
    @GET("status")
    suspend fun status(): StatusResponse

    @POST("account/setup")
    suspend fun setup(@Body request: PasswordRequest): Response<Unit>

    @POST("account/unlock")
    suspend fun unlock(@Body request: PasswordRequest): Response<Unit>

    @POST("account/lock")
    suspend fun lock(): Response<Unit>

    @GET("files")
    suspend fun listFiles(): List<FileInfo>

    @PUT("files/{name}")
    suspend fun putFile(@Path("name") name: String, @Body body: RequestBody): Response<Unit>

    @Streaming
    @GET("files/{name}")
    suspend fun getFile(@Path("name") name: String): ResponseBody

    @DELETE("files/{name}")
    suspend fun deleteFile(@Path("name") name: String): Response<Unit>
}
