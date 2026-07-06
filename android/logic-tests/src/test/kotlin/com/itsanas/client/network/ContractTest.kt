package com.itsanas.client.network

import kotlinx.coroutines.runBlocking
import kotlinx.serialization.json.Json
import okhttp3.mockwebserver.MockResponse
import okhttp3.mockwebserver.MockWebServer
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertTrue

/**
 * Pins the Android client's wire contract against literal JSON shaped
 * exactly like itsanas-daemon's real responses (see itsanas-daemon's
 * http.rs) — if either side ever renames a field, this fails immediately
 * instead of silently breaking the app the next time someone actually
 * runs it on a phone.
 */
class ContractTest {
    private val json = Json { ignoreUnknownKeys = true }

    @Test
    fun `StatusResponse deserializes the daemon's actual field names`() {
        val wire =
            """{"has_account":true,"unlocked":false,"synced_folder":"/home/alice/ITSaNAS","vault_health":null}"""

        val status = json.decodeFromString(StatusResponse.serializer(), wire)

        assertEquals(true, status.hasAccount)
        assertEquals(false, status.unlocked)
        assertEquals("/home/alice/ITSaNAS", status.syncedFolder)
        assertEquals(null, status.vaultHealth)
    }

    @Test
    fun `StatusResponse tolerates a response with no vault_health key at all`() {
        // Defends against a daemon build old enough to predate this field -
        // vaultHealth must default to null rather than failing to parse.
        val wire = """{"has_account":true,"unlocked":true,"synced_folder":"/home/alice/ITSaNAS"}"""

        val status = json.decodeFromString(StatusResponse.serializer(), wire)

        assertEquals(null, status.vaultHealth)
    }

    @Test
    fun `StatusResponse deserializes a populated vault_health exactly like the daemon sends it`() {
        val wire =
            """{"has_account":true,"unlocked":true,"synced_folder":"/home/alice/ITSaNAS",""" +
                """"vault_health":{"healthy_shards":5,"unhealthy_files":["notes.txt","photo.jpg"]}}"""

        val status = json.decodeFromString(StatusResponse.serializer(), wire)

        val health = status.vaultHealth
        checkNotNull(health)
        assertEquals(5L, health.healthyShards)
        assertEquals(listOf("notes.txt", "photo.jpg"), health.unhealthyFiles)
    }

    @Test
    fun `FileInfo deserializes the daemon's actual field names`() {
        val wire = """{"name":"notes.txt","size":42}"""

        val file = json.decodeFromString(FileInfo.serializer(), wire)

        assertEquals("notes.txt", file.name)
        assertEquals(42L, file.size)
    }

    @Test
    fun `FileInfo list deserializes the daemon's GET files shape`() {
        val wire = """[{"name":"a.txt","size":3},{"name":"b.txt","size":5}]"""

        val files = json.decodeFromString(
            kotlinx.serialization.builtins.ListSerializer(FileInfo.serializer()),
            wire,
        )

        assertEquals(2, files.size)
        assertEquals("a.txt", files[0].name)
        assertEquals("b.txt", files[1].name)
    }

    @Test
    fun `PasswordRequest serializes to the field name the daemon expects`() {
        val encoded = json.encodeToString(PasswordRequest.serializer(), PasswordRequest("hunter2"))

        assertEquals("""{"password":"hunter2"}""", encoded)
    }

    @Test
    fun `RetrofitClient accepts a base url with or without a trailing slash`() {
        // Retrofit itself throws IllegalArgumentException at build time if
        // a base URL isn't usable — not throwing here is the whole test.
        RetrofitClient.create("http://127.0.0.1:4279")
        RetrofitClient.create("http://127.0.0.1:4279/")
    }

    @Test
    fun `status() round-trips through a real HTTP call to the expected path`() {
        val server = MockWebServer()
        server.enqueue(
            MockResponse()
                .setBody("""{"has_account":true,"unlocked":true,"synced_folder":"/home/bob/ITSaNAS"}""")
                .addHeader("Content-Type", "application/json"),
        )
        server.start()

        try {
            val api = RetrofitClient.create(server.url("/").toString())
            val status = runBlocking { api.status() }

            assertTrue(status.hasAccount)
            assertTrue(status.unlocked)
            assertEquals("/home/bob/ITSaNAS", status.syncedFolder)

            val recorded = server.takeRequest()
            assertEquals("/status", recorded.path)
            assertEquals("GET", recorded.method)
        } finally {
            server.shutdown()
        }
    }

    @Test
    fun `setup() sends the password as the request body the daemon expects`() {
        val server = MockWebServer()
        server.enqueue(MockResponse().setResponseCode(201))
        server.start()

        try {
            val api = RetrofitClient.create(server.url("/").toString())
            runBlocking { api.setup(PasswordRequest("correct horse battery staple")) }

            val recorded = server.takeRequest()
            assertEquals("/account/setup", recorded.path)
            assertEquals("POST", recorded.method)
            assertEquals(
                """{"password":"correct horse battery staple"}""",
                recorded.body.readUtf8(),
            )
        } finally {
            server.shutdown()
        }
    }
}
