import ExpoModulesCore
import UIKit
import UniformTypeIdentifiers

private let bookmarksDefaultsKey = "FilePickerBookmarks"

public class FilePickerModule: Module, FilePickingResultHandler {
  private var pickingPromise: Promise?
  private var pickerDelegate: FilePickingDelegate?

  public func definition() -> ModuleDefinition {
    Name("FilePicker")

    AsyncFunction("pickFile") { (promise: Promise) in
      guard self.pickingPromise == nil else {
        promise.reject("PICKING_IN_PROGRESS", "A file picker is already open.")
        return
      }
      guard let vc = self.appContext?.utilities?.currentViewController() else {
        promise.reject("NO_VIEW_CONTROLLER", "Could not find a view controller to present from.")
        return
      }

      // Open IN PLACE (asCopy: false): the returned URL points at the real
      // document, and a security-scoped bookmark keeps it readable across
      // launches — that is what makes external-change detection possible.
      var types: [UTType] = [.plainText, .text]
      if let md = UTType(filenameExtension: "md") { types.insert(md, at: 0) }
      if let markdown = UTType(filenameExtension: "markdown") { types.insert(markdown, at: 1) }

      let pickerVC = UIDocumentPickerViewController(forOpeningContentTypes: types, asCopy: false)
      let delegate = FilePickingDelegate(resultHandler: self)
      self.pickerDelegate = delegate
      self.pickingPromise = promise
      pickerVC.delegate = delegate
      pickerVC.presentationController?.delegate = delegate
      vc.present(pickerVC, animated: true)
    }.runOnQueue(.main)

    // ref = bookmarkKey returned by pickFile.
    AsyncFunction("readPickedFile") { (ref: String, promise: Promise) in
      do {
        let url = try resolveBookmark(ref)
        let accessed = url.startAccessingSecurityScopedResource()
        defer { if accessed { url.stopAccessingSecurityScopedResource() } }
        let content = try String(contentsOf: url, encoding: .utf8)
        promise.resolve(content)
      } catch {
        promise.reject("READ_FAILED", error.localizedDescription)
      }
    }

    // Modification time in ms since epoch; null whenever it cannot be trusted.
    AsyncFunction("statPickedFile") { (ref: String, promise: Promise) in
      do {
        let url = try resolveBookmark(ref)
        let accessed = url.startAccessingSecurityScopedResource()
        defer { if accessed { url.stopAccessingSecurityScopedResource() } }
        let values = try url.resourceValues(forKeys: [.contentModificationDateKey])
        if let date = values.contentModificationDate {
          promise.resolve(date.timeIntervalSince1970 * 1000)
        } else {
          promise.resolve(nil)
        }
      } catch {
        promise.resolve(nil)
      }
    }
  }

  func didPickFile(url: URL) {
    defer {
      pickerDelegate = nil
      pickingPromise = nil
    }
    guard let promise = pickingPromise else { return }

    let accessed = url.startAccessingSecurityScopedResource()
    defer { if accessed { url.stopAccessingSecurityScopedResource() } }

    do {
      let bookmark = try url.bookmarkData()
      let key = UUID().uuidString
      storeBookmark(bookmark, forKey: key)
      promise.resolve([
        "uri": url.absoluteString,
        "name": url.lastPathComponent,
        "bookmarkKey": key,
      ])
    } catch {
      promise.reject("BOOKMARK_FAILED", error.localizedDescription)
    }
  }

  func didCancelPicking() {
    pickingPromise?.resolve(nil)
    pickerDelegate = nil
    pickingPromise = nil
  }
}

// MARK: - Bookmark persistence

private func storeBookmark(_ data: Data, forKey key: String) {
  var bookmarks = UserDefaults.standard.dictionary(forKey: bookmarksDefaultsKey) as? [String: Data] ?? [:]
  bookmarks[key] = data
  UserDefaults.standard.set(bookmarks, forKey: bookmarksDefaultsKey)
}

private enum BookmarkError: Error, LocalizedError {
  case notFound
  var errorDescription: String? { "No bookmark stored for this file." }
}

/// Resolve a stored bookmark to a URL, refreshing (re-persisting) it when iOS
/// reports it stale. Throws when the bookmark is missing or unresolvable
/// (file deleted, permission revoked).
private func resolveBookmark(_ key: String) throws -> URL {
  guard
    let bookmarks = UserDefaults.standard.dictionary(forKey: bookmarksDefaultsKey) as? [String: Data],
    let data = bookmarks[key]
  else {
    throw BookmarkError.notFound
  }
  var stale = false
  let url = try URL(resolvingBookmarkData: data, bookmarkDataIsStale: &stale)
  if stale {
    // Best-effort refresh so the next resolve is fast; failures keep old data.
    let accessed = url.startAccessingSecurityScopedResource()
    defer { if accessed { url.stopAccessingSecurityScopedResource() } }
    if let fresh = try? url.bookmarkData() {
      storeBookmark(fresh, forKey: key)
    }
  }
  return url
}

// MARK: - Protocol + Delegate

protocol FilePickingResultHandler: AnyObject {
  func didPickFile(url: URL)
  func didCancelPicking()
}

class FilePickingDelegate: NSObject, UIDocumentPickerDelegate, UIAdaptivePresentationControllerDelegate {
  private weak var resultHandler: FilePickingResultHandler?

  init(resultHandler: FilePickingResultHandler) {
    self.resultHandler = resultHandler
  }

  func documentPicker(_ controller: UIDocumentPickerViewController, didPickDocumentsAt urls: [URL]) {
    guard let url = urls.first else { resultHandler?.didCancelPicking(); return }
    resultHandler?.didPickFile(url: url)
  }

  func documentPickerWasCancelled(_ controller: UIDocumentPickerViewController) {
    resultHandler?.didCancelPicking()
  }

  func presentationControllerDidDismiss(_ presentationController: UIPresentationController) {
    resultHandler?.didCancelPicking()
  }
}
