/**
 * Sample Kotlin file exercising all extractor features.
 */

package com.example.app

import kotlin.math.sqrt
import java.time.Instant

const val MAX_RETRIES = 3
const val APP_NAME = "SampleApp"

/** Represents a 2D point. */
data class Point(val x: Double, val y: Double) {
    fun distanceTo(other: Point): Double {
        val dx = x - other.x
        val dy = y - other.y
        return sqrt(dx * dx + dy * dy)
    }
}

/** Sealed class representing operation results. */
sealed class Result<out T> {
    data class Success<T>(val value: T) : Result<T>()
    data class Failure(val error: String) : Result<Nothing>()
    object Loading : Result<Nothing>()
}

/** Interface for all repositories. */
interface Repository<T> {
    suspend fun findById(id: String): T?
    suspend fun findAll(): List<T>
    fun count(): Int
}

/** Annotation for marking cacheable methods. */
@Target(AnnotationTarget.FUNCTION)
@Retention(AnnotationRetention.RUNTIME)
annotation class Cacheable(val ttl: Int = 60)

/** Abstract base entity with an ID. */
abstract class Entity(val id: String) {
    val createdAt: Instant = Instant.now()
    abstract fun validate(): Boolean
}

/** A user entity. */
class User(
    id: String,
    val name: String,
    private val email: String,
    internal val role: Role = Role.USER,
) : Entity(id) {

    var lastLogin: Instant? = null
        private set

    override fun validate(): Boolean {
        return name.isNotBlank() && email.contains("@")
    }

    @Cacheable(ttl = 300)
    suspend fun loadProfile(): Map<String, Any> {
        println("Loading profile for $name")
        return mapOf("name" to name, "role" to role)
    }

    companion object {
        fun guest(): User = User("0", "Guest", "guest@example.com", Role.GUEST)
    }
}

enum class Role {
    ADMIN,
    USER,
    GUEST,
}

/** Singleton logger. */
object Logger {
    fun info(message: String) {
        println("[INFO] $message")
    }

    fun error(message: String) {
        println("[ERROR] $message")
    }
}

/** Extension function on String. */
fun String.toSlug(): String {
    return this.lowercase().replace(" ", "-")
}

/** Top-level function using various features. */
fun processUser(repo: Repository<User>, userId: String): Result<User> {
    val count = repo.count()
    Logger.info("Repository has $count users")
    return Result.Success(User.guest())
}

protected fun helperFunction(): Unit {
    Logger.info("helper")
}
