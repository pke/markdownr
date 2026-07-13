package dev.dudesoft.folderpicker

import android.app.Activity
import android.content.Intent
import android.net.Uri
import android.provider.DocumentsContract
import expo.modules.kotlin.Promise
import expo.modules.kotlin.modules.Module
import expo.modules.kotlin.modules.ModuleDefinition

private val MARKDOWN_EXTENSIONS = setOf("md", "markdown", "txt")
private const val REQUEST_CODE = 0x464F4C44 // "FOLD"

class FolderPickerModule : Module() {
  private var pendingPromise: Promise? = null

  override fun definition() = ModuleDefinition {
    Name("FolderPicker")

    AsyncFunction("pickFolder") { promise: Promise ->
      if (pendingPromise != null) {
        promise.reject("PICKING_IN_PROGRESS", "A folder picker is already open.", null)
        return@AsyncFunction
      }
      val activity = appContext.activityProvider?.currentActivity
        ?: run { promise.reject("NO_ACTIVITY", "No activity available.", null); return@AsyncFunction }

      pendingPromise = promise

      val intent = Intent(Intent.ACTION_OPEN_DOCUMENT_TREE).apply {
        addFlags(Intent.FLAG_GRANT_READ_URI_PERMISSION or Intent.FLAG_GRANT_PERSISTABLE_URI_PERMISSION)
      }
      activity.startActivityForResult(intent, REQUEST_CODE)
    }

    AsyncFunction("restoreFolder") { folderUri: String, promise: Promise ->
      val activity = appContext.activityProvider?.currentActivity
        ?: run { promise.resolve(null); return@AsyncFunction }

      val uri = Uri.parse(folderUri)

      // Check persisted permissions
      val hasPermission = activity.contentResolver.persistedUriPermissions
        .any { it.uri == uri && it.isReadPermission }

      if (!hasPermission) {
        promise.resolve(null)
        return@AsyncFunction
      }

      try {
        val files = enumerateMarkdownFiles(activity, uri)
        promise.resolve(mapOf(
          "folderUri" to uri.toString(),
          "folderName" to getFolderName(activity, uri),
          "files" to files,
        ))
      } catch (e: Exception) {
        promise.resolve(null)
      }
    }

    OnActivityResult { _, payload ->
      if (payload.requestCode != REQUEST_CODE) return@OnActivityResult
      val promise = pendingPromise ?: return@OnActivityResult
      pendingPromise = null

      if (payload.resultCode != Activity.RESULT_OK || payload.data == null) {
        promise.resolve(null)
        return@OnActivityResult
      }

      val uri = payload.data!!.data ?: run { promise.resolve(null); return@OnActivityResult }
      val activity = appContext.activityProvider?.currentActivity
        ?: run { promise.resolve(null); return@OnActivityResult }

      // Take persistent permission so we can restore on next launch
      activity.contentResolver.takePersistableUriPermission(
        uri,
        Intent.FLAG_GRANT_READ_URI_PERMISSION
      )

      try {
        val files = enumerateMarkdownFiles(activity, uri)
        promise.resolve(mapOf(
          "folderUri" to uri.toString(),
          "folderName" to getFolderName(activity, uri),
          "files" to files,
        ))
      } catch (e: Exception) {
        promise.reject("ENUMERATE_FAILED", e.message, e)
      }
    }
  }

  private fun getFolderName(activity: Activity, treeUri: Uri): String {
    val docId = DocumentsContract.getTreeDocumentId(treeUri)
    return docId.substringAfterLast('/').substringAfterLast(':').ifEmpty { "Folder" }
  }

  private fun enumerateMarkdownFiles(activity: Activity, treeUri: Uri): List<Map<String, String>> {
    val results = mutableListOf<Pair<String, Uri>>() // relativePath -> fileUri
    enumerateDir(activity, treeUri, DocumentsContract.getTreeDocumentId(treeUri), "", results)
    return results
      .sortedBy { it.first }
      .map { (relativePath, uri) ->
        mapOf(
          "name" to relativePath.substringAfterLast('/'),
          "uri" to uri.toString(),
          "relativePath" to relativePath,
        )
      }
  }

  private fun enumerateDir(
    activity: Activity,
    treeUri: Uri,
    parentDocId: String,
    pathPrefix: String,
    results: MutableList<Pair<String, Uri>>,
  ) {
    val childrenUri = DocumentsContract.buildChildDocumentsUriUsingTree(treeUri, parentDocId)
    val projection = arrayOf(
      DocumentsContract.Document.COLUMN_DOCUMENT_ID,
      DocumentsContract.Document.COLUMN_DISPLAY_NAME,
      DocumentsContract.Document.COLUMN_MIME_TYPE,
    )
    activity.contentResolver.query(childrenUri, projection, null, null, null)?.use { cursor ->
      val idCol = cursor.getColumnIndexOrThrow(DocumentsContract.Document.COLUMN_DOCUMENT_ID)
      val nameCol = cursor.getColumnIndexOrThrow(DocumentsContract.Document.COLUMN_DISPLAY_NAME)
      val mimeCol = cursor.getColumnIndexOrThrow(DocumentsContract.Document.COLUMN_MIME_TYPE)

      while (cursor.moveToNext()) {
        val docId = cursor.getString(idCol)
        val name = cursor.getString(nameCol) ?: continue
        val mime = cursor.getString(mimeCol) ?: continue
        val relativePath = if (pathPrefix.isEmpty()) name else "$pathPrefix/$name"

        if (mime == DocumentsContract.Document.MIME_TYPE_DIR) {
          // Recurse into subdirectory
          enumerateDir(activity, treeUri, docId, relativePath, results)
        } else {
          val ext = name.substringAfterLast('.', "").lowercase()
          if (ext in MARKDOWN_EXTENSIONS) {
            val fileUri = DocumentsContract.buildDocumentUriUsingTree(treeUri, docId)
            results.add(relativePath to fileUri)
          }
        }
      }
    }
  }
}
