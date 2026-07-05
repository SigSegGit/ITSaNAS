package com.itsanas.client.network

import android.content.Context
import android.net.Uri
import android.provider.OpenableColumns
import java.io.IOException
import okhttp3.MediaType.Companion.toMediaType
import okhttp3.RequestBody
import okio.BufferedSink
import okio.source

/**
 * Streams a content [Uri] straight into the request body instead of
 * reading it fully into memory first — matters here since the daemon's
 * own body-size limit is deliberately unbounded (see itsanas-daemon's
 * http.rs), so a phone photo/video library shouldn't have a lower ceiling
 * than the API does.
 */
fun uriRequestBody(context: Context, uri: Uri): RequestBody =
    object : RequestBody() {
        override fun contentType() = "application/octet-stream".toMediaType()

        override fun contentLength(): Long =
            context.contentResolver.query(uri, null, null, null, null)?.use { cursor ->
                val sizeIndex = cursor.getColumnIndex(OpenableColumns.SIZE)
                if (sizeIndex >= 0 && cursor.moveToFirst()) cursor.getLong(sizeIndex) else -1L
            } ?: -1L

        override fun writeTo(sink: BufferedSink) {
            val input =
                context.contentResolver.openInputStream(uri)
                    ?: throw IOException("couldn't open $uri")
            input.use { stream -> sink.writeAll(stream.source()) }
        }
    }
