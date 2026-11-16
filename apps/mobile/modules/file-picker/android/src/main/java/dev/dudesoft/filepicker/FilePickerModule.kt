package dev.dudesoft.filepicker

import android.app.Activity
import android.content.Intent
import android.net.Uri
import android.provider.OpenableColumns
import android.provider.DocumentsContract
import expo.modules.kotlin.Promise
import expo.modules.kotlin.modules.Module
import expo.modules.kotlin.modules.ModuleDefinition

private const val REQUEST_CODE = 0x46494C45 // "FILE"

class FilePickerModule : Module() {
  private var pendingPromise: Promise? = null

  override fun definition() = ModuleDefinition {
    Name("FilePicker")

    AsyncFunction("pickFile") { promise: Promise ->
      if (pendingPromise != null) {
        promise.reject("PICKING_IN_PROGRESS", "A file picker is already open.", null)
        return@AsyncFunction
      }
      val activity = appContext.activityProvider?.currentActivity
        ?: run { promise.reject("NO_ACTIVITY", "No activity available.", null); return@AsyncFunction }

      pendingPromise = promise

      val intent = Intent(Intent.ACTION_OPEN_DOCUMENT).apply {
        addCategory(Intent.CATEGORY_OPENABLE)
        type = "*/*"
        putExtra(Intent.EXTRA_MIME_TYPES, arrayOf("text/markdown", "text/plain"))
        addFlags(Intent.FLAG_GRANT_READ_URI_PERMISSION or Intent.FLAG_GRANT_PERSISTABLE_URI_PERMISSION)
      }
      activity.startActivityForResult(intent, REQUEST_CODE)
    }

    // ref = the content:// uri returned by pickFile.
    AsyncFunction("readPickedFile") { ref: String, promise: Promise ->
      val activity = appContext.activityProvider?.currentActivity
        ?: run { promise.reject("NO_ACTIVITY", "No activity available.", null); return@AsyncFunction }
      try {
        val uri = Uri.parse(ref)
        val content = activity.contentResolver.openInputStream(uri)?.use {
          it.readBytes().toString(Charsets.UTF_8)
        } ?: throw IllegalStateException("Could not open stream")
        promise.resolve(content)
      } catch (e: Exception) {
        promise.reject("READ_FAILED", e.message ?: "read failed", e)
      }
    }

    // Last-modified in ms since epoch; null whenever it cannot be trusted
    // (provider reports 0, permission gone, column missing).
    AsyncFunction("statPickedFile") { ref: String, promise: Promise ->
      val activity = appContext.activityProvider?.currentActivity
        ?: run { promise.resolve(null); return@AsyncFunction }
      try {
        val uri = Uri.parse(ref)
        activity.contentResolver.query(
          uri,
          arrayOf(DocumentsContract.Document.COLUMN_LAST_MODIFIED),
          null, null, null
        )?.use { cursor ->
          if (cursor.moveToFirst() && !cursor.isNull(0)) {
            val mtime = cursor.getLong(0)
            promise.resolve(if (mtime > 0) mtime else null)
          } else {
            promise.resolve(null)
          }
        } ?: promise.resolve(null)
      } catch (e: Exception) {
        promise.resolve(null)
      }
    }

    OnActivityResult { _, payload ->
      if (payload.requestCode != REQUEST_CODE) return@OnActivityResult
      val promise = pendingPromise ?: return@OnActivityResult
      pendingPromise = null

      val activity = appContext.activityProvider?.currentActivity
      val uri = payload.data?.data
      if (payload.resultCode != Activity.RESULT_OK || uri == null || activity == null) {
        promise.resolve(null)
        return@OnActivityResult
      }

      try {
        // Keep the grant across relaunches — this is what makes the file
        // watchable (and re-readable) later.
        activity.contentResolver.takePersistableUriPermission(
          uri, Intent.FLAG_GRANT_READ_URI_PERMISSION
        )
      } catch (e: SecurityException) {
        // Provider doesn't support persisting; the file still opens now,
        // it just won't be watchable after a relaunch.
      }

      promise.resolve(mapOf(
        "uri" to uri.toString(),
        "name" to queryDisplayName(activity, uri),
      ))
    }
  }
}

private fun queryDisplayName(activity: Activity, uri: Uri): String {
  return try {
    activity.contentResolver.query(uri, arrayOf(OpenableColumns.DISPLAY_NAME), null, null, null)
      ?.use { cursor ->
        if (cursor.moveToFirst() && !cursor.isNull(0)) cursor.getString(0) else null
      } ?: uri.lastPathSegment ?: "Unknown"
  } catch (e: Exception) {
    uri.lastPathSegment ?: "Unknown"
  }
}
