package dev.shulhi.rex

object NativeBridge {
    init { System.loadLibrary("rex_android") }
    @JvmStatic external fun create(): Long
    @JvmStatic external fun command(handle: Long, request: String): String
    @JvmStatic external fun poll(handle: Long, after: Long): String
    @JvmStatic external fun awaitUpdate(handle: Long, after: Long, timeoutMs: Long): Long
    @JvmStatic external fun readUpdate(handle: Long, after: Long): ByteArray
    /** Copies the next update into a direct buffer: its length, 0 for none, or the negated length when the buffer is too small. */
    @JvmStatic external fun readUpdateInto(handle: Long, after: Long, buffer: java.nio.ByteBuffer): Int
    /** Whether every RGBA pixel in this stretch of a direct buffer is fully opaque. */
    @JvmStatic external fun opaquePixels(buffer: java.nio.ByteBuffer, offset: Int, length: Int): Boolean
    @JvmStatic external fun uploadCreate(request: String): Long
    @JvmStatic external fun uploadPoll(handle: Long): String
    @JvmStatic external fun uploadDestroy(handle: Long)
    @JvmStatic external fun forwardCreate(request: String): Long
    @JvmStatic external fun forwardPoll(handle: Long): String
    @JvmStatic external fun forwardDestroy(handle: Long)
    @JvmStatic external fun networkCommand(request: String): String
    @JvmStatic external fun destroy(handle: Long)
}
