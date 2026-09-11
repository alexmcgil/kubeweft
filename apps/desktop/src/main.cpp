#include <QDir>
#include <QGuiApplication>
#include <QQmlApplicationEngine>
#include <QQmlError>
#include <QQmlContext>
#include <QStandardPaths>
#include <QFile>

#include <cstdio>

#include "filesystembridge.h"

int main(int argc, char *argv[]) {
  QGuiApplication application(argc, argv);
  QCoreApplication::setApplicationName(QStringLiteral("Kubeweft"));
  QCoreApplication::setOrganizationName(QStringLiteral("Kubeweft"));

  const QString dataDirectory = qEnvironmentVariableIsSet("KUBEWEFT_DATA_DIR")
                                    ? qEnvironmentVariable("KUBEWEFT_DATA_DIR")
                                    : QStandardPaths::writableLocation(
                                          QStandardPaths::GenericDataLocation) +
                                          QStringLiteral("/kubeweft");
  QDir().mkpath(dataDirectory);

  FilesystemBridge filesystem(dataDirectory);
  QQmlApplicationEngine engine;
  QObject::connect(&engine, &QQmlApplicationEngine::warnings,
                   [](const QList<QQmlError> &warnings) {
                     for (const QQmlError &warning : warnings) {
                       qWarning().noquote() << warning.toString();
                     }
                   });
  engine.rootContext()->setContextProperty(QStringLiteral("filesystem"),
                                           &filesystem);
  engine.load(QUrl(QStringLiteral("qrc:/qt/qml/Kubeweft/Main.qml")));
  if (engine.rootObjects().isEmpty()) {
    std::fprintf(stderr, "desktop QML failed to load (resource exists: %s)\n",
                 QFile::exists(QStringLiteral(":/qt/qml/Kubeweft/Main.qml"))
                     ? "yes"
                     : "no");
    return 1;
  }
  return application.exec();
}
