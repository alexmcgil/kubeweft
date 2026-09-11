#include "filesystembridge.h"

#include <QDir>
#include <QJsonArray>
#include <QJsonDocument>
#include <QJsonObject>

FilesystemBridge::FilesystemBridge(const QString &dataDirectory, QObject *parent)
    : QObject(parent) {
  const QByteArray encoded = dataDirectory.toUtf8();
  filesystem_ = kubeweft_desktop_open(encoded.constData());
  if (filesystem_ == nullptr) {
    setStatus(QStringLiteral("Could not open local Kubeweft data"));
    return;
  }
  refresh();
}

FilesystemBridge::~FilesystemBridge() {
  kubeweft_desktop_close(filesystem_);
}

void FilesystemBridge::refresh() {
  if (filesystem_ == nullptr) {
    return;
  }
  QJsonObject value;
  const QByteArray path = currentPath_.toUtf8();
  if (!applyResult(kubeweft_desktop_list(filesystem_, path.constData()),
                   &value)) {
    return;
  }

  QVariantList entries;
  for (const QJsonValue &entry : value.value(QStringLiteral("entries")).toArray()) {
    entries.append(entry.toObject().toVariantMap());
  }
  entries_ = entries;
  emit entriesChanged();
  setStatus(QStringLiteral("Ready"));
}

void FilesystemBridge::openEntry(const QString &name, const QString &kind) {
  const QString path = childPath(name);
  if (kind == QStringLiteral("directory")) {
    currentPath_ = path;
    clearSelection();
    emit currentPathChanged();
    refresh();
    return;
  }

  QJsonObject value;
  const QByteArray encoded = path.toUtf8();
  if (!applyResult(kubeweft_desktop_read(filesystem_, encoded.constData()),
                   &value)) {
    return;
  }
  selectedPath_ = path;
  content_ = value.value(QStringLiteral("content")).toString();
  generation_ = value.value(QStringLiteral("generation")).toInteger();
  emit selectedFileChanged();
  setStatus(QStringLiteral("Opened %1").arg(name));
}

void FilesystemBridge::up() {
  if (currentPath_ == QStringLiteral("/")) {
    return;
  }
  const int separator = currentPath_.lastIndexOf('/');
  currentPath_ = separator == 0 ? QStringLiteral("/")
                                : currentPath_.left(separator);
  clearSelection();
  emit currentPathChanged();
  refresh();
}

void FilesystemBridge::createDirectory(const QString &name) {
  if (name.trimmed().isEmpty()) {
    setStatus(QStringLiteral("Enter a directory name"));
    return;
  }
  const QByteArray path = childPath(name.trimmed()).toUtf8();
  if (applyResult(kubeweft_desktop_mkdir(filesystem_, path.constData()))) {
    refresh();
  }
}

void FilesystemBridge::createFile(const QString &name) {
  if (name.trimmed().isEmpty()) {
    setStatus(QStringLiteral("Enter a file name"));
    return;
  }
  const QString path = childPath(name.trimmed());
  const QByteArray encoded = path.toUtf8();
  if (!applyResult(kubeweft_desktop_create(filesystem_, encoded.constData()))) {
    return;
  }
  refresh();
  openEntry(name.trimmed(), QStringLiteral("file"));
}

void FilesystemBridge::save(const QString &content) {
  if (selectedPath_.isEmpty()) {
    setStatus(QStringLiteral("Select a file first"));
    return;
  }
  const QByteArray path = selectedPath_.toUtf8();
  const QByteArray encoded = content.toUtf8();
  QJsonObject value;
  if (!applyResult(kubeweft_desktop_write(filesystem_, path.constData(),
                                          encoded.constData(), generation_),
                   &value)) {
    return;
  }
  content_ = content;
  generation_ = value.value(QStringLiteral("generation")).toInteger();
  emit selectedFileChanged();
  refresh();
  setStatus(QStringLiteral("Saved generation %1").arg(generation_));
}

void FilesystemBridge::removeSelected() {
  if (selectedPath_.isEmpty()) {
    setStatus(QStringLiteral("Select a file first"));
    return;
  }
  const QByteArray path = selectedPath_.toUtf8();
  if (applyResult(kubeweft_desktop_remove(filesystem_, path.constData()))) {
    clearSelection();
    refresh();
  }
}

QString FilesystemBridge::childPath(const QString &name) const {
  return currentPath_ == QStringLiteral("/") ? QStringLiteral("/") + name
                                              : currentPath_ + '/' + name;
}

bool FilesystemBridge::applyResult(char *encoded, QJsonObject *value) {
  if (encoded == nullptr) {
    setStatus(QStringLiteral("Empty response from filesystem bridge"));
    return false;
  }
  const QByteArray bytes(encoded);
  kubeweft_desktop_string_free(encoded);
  QJsonParseError parseError;
  const QJsonDocument document = QJsonDocument::fromJson(bytes, &parseError);
  if (parseError.error != QJsonParseError::NoError || !document.isObject()) {
    setStatus(QStringLiteral("Invalid response from filesystem bridge"));
    return false;
  }
  const QJsonObject response = document.object();
  if (!response.value(QStringLiteral("ok")).toBool()) {
    setStatus(response.value(QStringLiteral("error")).toString());
    return false;
  }
  if (value != nullptr) {
    *value = response.value(QStringLiteral("value")).toObject();
  }
  return true;
}

void FilesystemBridge::setStatus(const QString &status) {
  if (status_ == status) {
    return;
  }
  status_ = status;
  emit statusChanged();
}

void FilesystemBridge::clearSelection() {
  selectedPath_.clear();
  content_.clear();
  generation_ = 0;
  emit selectedFileChanged();
}
