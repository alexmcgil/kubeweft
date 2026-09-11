#pragma once

#include <QObject>
#include <QString>
#include <QVariantList>

#include "bridge.h"

class QJsonObject;

class FilesystemBridge final : public QObject {
  Q_OBJECT
  Q_PROPERTY(QString currentPath READ currentPath NOTIFY currentPathChanged)
  Q_PROPERTY(QVariantList entries READ entries NOTIFY entriesChanged)
  Q_PROPERTY(QString selectedPath READ selectedPath NOTIFY selectedFileChanged)
  Q_PROPERTY(QString content READ content NOTIFY selectedFileChanged)
  Q_PROPERTY(qulonglong generation READ generation NOTIFY selectedFileChanged)
  Q_PROPERTY(QString status READ status NOTIFY statusChanged)

public:
  explicit FilesystemBridge(const QString &dataDirectory,
                            QObject *parent = nullptr);
  ~FilesystemBridge() override;

  QString currentPath() const { return currentPath_; }
  QVariantList entries() const { return entries_; }
  QString selectedPath() const { return selectedPath_; }
  QString content() const { return content_; }
  qulonglong generation() const { return generation_; }
  QString status() const { return status_; }

  Q_INVOKABLE void refresh();
  Q_INVOKABLE void openEntry(const QString &name, const QString &kind);
  Q_INVOKABLE void up();
  Q_INVOKABLE void createDirectory(const QString &name);
  Q_INVOKABLE void createFile(const QString &name);
  Q_INVOKABLE void save(const QString &content);
  Q_INVOKABLE void removeSelected();

signals:
  void currentPathChanged();
  void entriesChanged();
  void selectedFileChanged();
  void statusChanged();

private:
  QString childPath(const QString &name) const;
  bool applyResult(char *encoded, QJsonObject *value = nullptr);
  void setStatus(const QString &status);
  void clearSelection();

  DesktopFilesystem *filesystem_ = nullptr;
  QString currentPath_ = QStringLiteral("/home/user");
  QVariantList entries_;
  QString selectedPath_;
  QString content_;
  qulonglong generation_ = 0;
  QString status_;
};
