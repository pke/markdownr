import ExpoModulesCore
import UIKit
import UniformTypeIdentifiers

private let markdownExtensions: Set<String> = ["md", "markdown", "txt"]

public class FolderPickerModule: Module, FolderPickingResultHandler {
  private var pickingPromise: Promise?
  private var pickerDelegate: FolderPickingDelegate?

  public func definition() -> ModuleDefinition {
    Name("FolderPicker")

    AsyncFunction("pickFolder") { (promise: Promise) in
      guard self.pickingPromise == nil else {
        promise.reject("PICKING_IN_PROGRESS", "A folder picker is already open.")
        return
      }
      guard let vc = self.appContext?.utilities?.currentViewController() else {
        promise.reject("NO_VIEW_CONTROLLER", "Could not find a view controller to present from.")
        return
      }

      guard #available(iOS 14.0, *) else {
        promise.reject("UNSUPPORTED", "Folder picking requires iOS 14+.")
        return
      }

      let pickerVC = UIDocumentPickerViewController(forOpeningContentTypes: [.folder])
      let delegate = FolderPickingDelegate(resultHandler: self)
      self.pickerDelegate = delegate
      self.pickingPromise = promise
      pickerVC.delegate = delegate
      pickerVC.presentationController?.delegate = delegate
      vc.present(pickerVC, animated: true)
    }.runOnQueue(.main)

    AsyncFunction("restoreFolder") { (folderUri: String, promise: Promise) in
      // folderUri is a cache directory URI we created — always accessible
      guard let url = URL(string: folderUri) else {
        promise.resolve(nil)
        return
      }
      do {
        let files = try enumerateCachedFiles(in: url)
        let result: [String: Any] = [
          "folderUri": url.absoluteString,
          "folderName": url.lastPathComponent,
          "files": files.map { fileToDict($0, base: url) },
        ]
        promise.resolve(result)
      } catch {
        promise.resolve(nil)
      }
    }
  }

  func didPickFolder(url: URL) {
    defer {
      pickerDelegate = nil
      pickingPromise = nil
    }
    guard let promise = pickingPromise else { return }

    // Start security-scoped access
    let accessed = url.startAccessingSecurityScopedResource()

    do {
      // Copy entire folder into app cache while scope is active
      let cacheDir = try copyFolderToCache(url)
      if accessed { url.stopAccessingSecurityScopedResource() }

      let files = try enumerateCachedFiles(in: cacheDir)
      let result: [String: Any] = [
        "folderUri": cacheDir.absoluteString,
        "folderName": url.lastPathComponent,
        "files": files.map { fileToDict($0, base: cacheDir) },
      ]
      promise.resolve(result)
    } catch {
      if accessed { url.stopAccessingSecurityScopedResource() }
      promise.reject("COPY_FAILED", error.localizedDescription)
    }
  }

  func didCancelPicking() {
    pickingPromise?.resolve(nil)
    pickerDelegate = nil
    pickingPromise = nil
  }
}

// MARK: - Helpers

/// Copy the picked folder into a stable cache location and return its URL.
private func copyFolderToCache(_ sourceURL: URL) throws -> URL {
  let fm = FileManager.default
  let cacheRoot = try fm.url(
    for: .cachesDirectory,
    in: .userDomainMask,
    appropriateFor: nil,
    create: true
  ).appendingPathComponent("FolderPicker", isDirectory: true)

  // Remove previous copy of same folder name to avoid stale files
  let destURL = cacheRoot.appendingPathComponent(sourceURL.lastPathComponent, isDirectory: true)
  if fm.fileExists(atPath: destURL.path) {
    try fm.removeItem(at: destURL)
  }
  try fm.createDirectory(at: cacheRoot, withIntermediateDirectories: true)
  try fm.copyItem(at: sourceURL, to: destURL)
  return destURL
}

/// Enumerate all markdown files in a (cached, always-accessible) directory.
private func enumerateCachedFiles(in directory: URL) throws -> [URL] {
  let fm = FileManager.default
  guard let enumerator = fm.enumerator(
    at: directory,
    includingPropertiesForKeys: [.isRegularFileKey],
    options: [.skipsHiddenFiles]
  ) else {
    return []
  }

  var files: [URL] = []
  for case let fileURL as URL in enumerator {
    let ext = fileURL.pathExtension.lowercased()
    if markdownExtensions.contains(ext) {
      let resources = try fileURL.resourceValues(forKeys: [.isRegularFileKey])
      if resources.isRegularFile == true {
        files.append(fileURL)
      }
    }
  }
  return files.sorted { $0.path < $1.path }
}

private func fileToDict(_ url: URL, base: URL) -> [String: String] {
  let relativePath = String(url.path.dropFirst(base.path.count + 1))
  return [
    "name": url.lastPathComponent,
    "uri": url.absoluteString,
    "relativePath": relativePath,
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
