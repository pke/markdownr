import ExpoModulesCore
import UIKit
import UniformTypeIdentifiers

private let markdownExtensions: Set<String> = ["md", "markdown", "txt"]
private let bookmarkPrefix = "FolderPickerBookmark:"

public class FolderPickerModule: Module, FolderPickingResultHandler {
  private var pickingPromise: Promise?
  private var pickerDelegate: FolderPickingDelegate?
  private let fileQueue = DispatchQueue(label: "dev.dudesoft.folderpicker")
  private var watchedBookmark: String?
  private var watchGeneration = 0
  private var presenter: FolderPresenter?
  private var presentedScope: URL?

  public func definition() -> ModuleDefinition {
    Name("FolderPicker")
    Events("onFolderChanged")

    AsyncFunction("pickFolder") { (promise: Promise) in
      guard self.pickingPromise == nil else {
        promise.reject("PICKING_IN_PROGRESS", "A folder picker is already open.")
        return
      }
      guard let vc = self.appContext?.utilities?.currentViewController() else {
        promise.reject("NO_VIEW_CONTROLLER", "Could not find a view controller to present from.")
        return
      }

      let pickerVC = UIDocumentPickerViewController(forOpeningContentTypes: [.folder], asCopy: false)
      let delegate = FolderPickingDelegate(resultHandler: self)
      self.pickerDelegate = delegate
      self.pickingPromise = promise
      pickerVC.delegate = delegate
      pickerVC.presentationController?.delegate = delegate
      vc.present(pickerVC, animated: true)
    }.runOnQueue(.main)

    AsyncFunction("restoreFolder") { (key: String) -> [String: Any]? in
      try? withSecurityScope(resolveBookmark(key)) { url in
        try folderResult(url, bookmarkKey: key)
      }
    }.runOnQueue(fileQueue)

    AsyncFunction("readFolderFile") { (key: String, relativePath: String) -> String in
      try withFolderFile(key, relativePath: relativePath) { url in
        try String(contentsOf: url, encoding: .utf8)
      }
    }.runOnQueue(fileQueue)

    AsyncFunction("statFolderFile") { (key: String, relativePath: String) -> Double? in
      try? withFolderFile(key, relativePath: relativePath) { url in
        let values = try url.resourceValues(forKeys: [.contentModificationDateKey])
        return values.contentModificationDate.map { $0.timeIntervalSince1970 * 1000 }
      }
    }.runOnQueue(fileQueue)

    AsyncFunction("watchFolder") { (key: String?) in
      self.watchedBookmark = key
      if UIApplication.shared.applicationState == .background {
        self.stopPresenting()
      } else {
        self.startPresenting()
      }
    }.runOnQueue(.main)

    OnAppEntersBackground {
      self.stopPresenting()
    }
    OnAppEntersForeground {
      self.startPresenting()
    }
    OnDestroy {
      DispatchQueue.main.async {
        self.watchedBookmark = nil
        self.stopPresenting()
      }
    }
  }

  func didPickFolder(url: URL) {
    guard let promise = pickingPromise else { return }
    fileQueue.async {
      do {
        let result = try withSecurityScope(url) { url in
          let path = url.standardizedFileURL.path
          let key = UserDefaults.standard.string(forKey: bookmarkPrefix + "path:" + path) ?? path
          let result = try folderResult(url, bookmarkKey: key)
          try saveBookmark(url, key: key)
          return result
        }
        DispatchQueue.main.async {
          self.finishPicking()
          promise.resolve(result)
        }
      } catch {
        DispatchQueue.main.async {
          self.finishPicking()
          promise.reject("FOLDER_ACCESS_FAILED", error.localizedDescription)
        }
      }
    }
    pickerDelegate = nil
  }

  func didCancelPicking() {
    pickingPromise?.resolve(nil)
    finishPicking()
  }

  private func finishPicking() {
    pickerDelegate = nil
    pickingPromise = nil
  }

  private func stopPresenting() {
    watchGeneration += 1
    if let presenter {
      NSFileCoordinator.removeFilePresenter(presenter)
    }
    presentedScope?.stopAccessingSecurityScopedResource()
    presentedScope = nil
    presenter = nil
  }

  private func startPresenting() {
    stopPresenting()
    guard let key = watchedBookmark else { return }
    let generation = watchGeneration
    fileQueue.async { [weak self] in
      guard let url = try? resolveBookmark(key) else { return }
      DispatchQueue.main.async { [weak self] in
        guard let self, self.watchGeneration == generation, url.startAccessingSecurityScopedResource() else { return }
        let presenter = FolderPresenter(url: url) { [weak self] in
          guard let self, self.watchGeneration == generation else { return }
          self.sendEvent("onFolderChanged", ["bookmarkKey": key])
        }
        self.presentedScope = url
        self.presenter = presenter
        NSFileCoordinator.addFilePresenter(presenter)
      }
    }
  }
}

private final class FolderPresenter: NSObject, NSFilePresenter {
  var presentedItemURL: URL?
  let presentedItemOperationQueue = OperationQueue.main
  private let changed: () -> Void

  init(url: URL, changed: @escaping () -> Void) {
    self.presentedItemURL = url
    self.changed = changed
  }

  func presentedItemDidChange() { changed() }
  func presentedSubitemDidAppear(at url: URL) { changed() }
  func presentedSubitemDidChange(at url: URL) { changed() }
  func presentedSubitem(at oldURL: URL, didMoveTo newURL: URL) { changed() }

  func presentedItemDidMove(to newURL: URL) {
    presentedItemURL = newURL
    changed()
  }

  func accommodatePresentedSubitemDeletion(at url: URL, completionHandler: @escaping (Error?) -> Void) {
    completionHandler(nil)
    changed()
  }

  func accommodatePresentedItemDeletion(completionHandler: @escaping (Error?) -> Void) {
    completionHandler(nil)
    changed()
  }
}

private func saveBookmark(_ url: URL, key: String) throws {
  let data = try url.bookmarkData(options: .minimalBookmark, includingResourceValuesForKeys: nil, relativeTo: nil)
  UserDefaults.standard.set(data, forKey: bookmarkPrefix + key)
  UserDefaults.standard.set(key, forKey: bookmarkPrefix + "path:" + url.standardizedFileURL.path)
}

private func resolveBookmark(_ key: String) throws -> URL {
  guard let data = UserDefaults.standard.data(forKey: bookmarkPrefix + key) else {
    throw CocoaError(.fileReadNoSuchFile)
  }
  var stale = false
  let url = try URL(resolvingBookmarkData: data, bookmarkDataIsStale: &stale)
  if stale {
    try? withSecurityScope(url) { try saveBookmark($0, key: key) }
  }
  return url
}

private func withSecurityScope<T>(_ url: URL, _ access: (URL) throws -> T) throws -> T {
  guard url.startAccessingSecurityScopedResource() else {
    throw CocoaError(.fileReadNoPermission)
  }
  defer { url.stopAccessingSecurityScopedResource() }
  return try access(url)
}

private func coordinatedRead<T>(_ url: URL, _ read: (URL) throws -> T) throws -> T {
  var error: NSError?
  var result: Result<T, Error>?
  NSFileCoordinator().coordinate(readingItemAt: url, options: .withoutChanges, error: &error) { url in
    result = Result { try read(url) }
  }
  if let error { throw error }
  guard let result else { throw CocoaError(.fileReadUnknown) }
  return try result.get()
}

private func withFolderFile<T>(_ key: String, relativePath: String, _ read: (URL) throws -> T) throws -> T {
  let parts = relativePath.split(separator: "/", omittingEmptySubsequences: false)
  guard parts.allSatisfy({ !$0.isEmpty && $0 != "." && $0 != ".." }) else {
    throw CocoaError(.fileReadInvalidFileName)
  }

  return try withSecurityScope(resolveBookmark(key)) { folder in
    let file = parts.reduce(folder) { $0.appendingPathComponent(String($1)) }
    let folderPath = folder.resolvingSymlinksInPath().path
    guard file.resolvingSymlinksInPath().path.hasPrefix(folderPath + "/") else {
      throw CocoaError(.fileReadNoPermission)
    }
    return try coordinatedRead(file, read)
  }
}

private func folderResult(_ folder: URL, bookmarkKey: String) throws -> [String: Any] {
  let files = try coordinatedRead(folder) { folder -> [[String: String]] in
    let keys: Set<URLResourceKey> = [.isRegularFileKey, .isSymbolicLinkKey]
    var enumerationError: Error?
    let enumerator = FileManager.default.enumerator(
      at: folder,
      includingPropertiesForKeys: Array(keys),
      options: [.skipsHiddenFiles],
      errorHandler: { _, error in
        enumerationError = error
        return false
      }
    )
    guard let enumerator else { throw CocoaError(.fileReadUnknown) }

    var files: [[String: String]] = []
    let prefix = folder.path.hasSuffix("/") ? folder.path : folder.path + "/"
    for case let file as URL in enumerator {
      guard markdownExtensions.contains(file.pathExtension.lowercased()),
        file.path.hasPrefix(prefix),
        let values = try? file.resourceValues(forKeys: keys),
        values.isRegularFile == true,
        values.isSymbolicLink != true
      else { continue }
      files.append([
        "name": file.lastPathComponent,
        "uri": file.absoluteString,
        "relativePath": String(file.path.dropFirst(prefix.count)),
      ])
    }
    if let enumerationError { throw enumerationError }
    return files.sorted { $0["relativePath", default: ""] < $1["relativePath", default: ""] }
  }

  return [
    "folderUri": folder.absoluteString,
    "folderName": folder.lastPathComponent,
    "bookmarkKey": bookmarkKey,
    "files": files,
  ]
}

// MARK: - Protocol + Delegate

protocol FolderPickingResultHandler: AnyObject {
  func didPickFolder(url: URL)
  func didCancelPicking()
}

class FolderPickingDelegate: NSObject, UIDocumentPickerDelegate, UIAdaptivePresentationControllerDelegate {
  private weak var resultHandler: FolderPickingResultHandler?

  init(resultHandler: FolderPickingResultHandler) {
    self.resultHandler = resultHandler
  }

  func documentPicker(_ controller: UIDocumentPickerViewController, didPickDocumentsAt urls: [URL]) {
    guard let url = urls.first else { resultHandler?.didCancelPicking(); return }
    resultHandler?.didPickFolder(url: url)
  }

  func documentPickerWasCancelled(_ controller: UIDocumentPickerViewController) {
    resultHandler?.didCancelPicking()
  }

  func presentationControllerDidDismiss(_ presentationController: UIPresentationController) {
    resultHandler?.didCancelPicking()
  }
}
