import QtQuick
import QtQuick.Window

Window {
    width: 1040
    height: 680
    visible: true
    title: "Kubeweft — " + filesystem.currentPath
    color: "#0f172a"

    Rectangle {
        id: header
        anchors.left: parent.left
        anchors.right: parent.right
        anchors.top: parent.top
        height: 58
        color: "#111827"

        KButton {
            id: upButton
            anchors.left: parent.left
            anchors.leftMargin: 16
            anchors.verticalCenter: parent.verticalCenter
            text: "Up"
            onClicked: filesystem.up()
        }

        Text {
            anchors.left: upButton.right
            anchors.leftMargin: 18
            anchors.verticalCenter: parent.verticalCenter
            text: filesystem.currentPath
            color: "#f8fafc"
            font.pixelSize: 18
            font.bold: true
        }
    }

    Rectangle {
        id: browser
        anchors.left: parent.left
        anchors.top: header.bottom
        anchors.bottom: status.top
        width: 390
        color: "#111827"

        ListView {
            id: fileList
            anchors.left: parent.left
            anchors.right: parent.right
            anchors.top: parent.top
            anchors.bottom: createRow.top
            anchors.margins: 12
            spacing: 4
            clip: true
            model: filesystem.entries

            delegate: Rectangle {
                required property var modelData
                width: fileList.width
                height: 44
                radius: 6
                color: rowMouse.containsMouse ? "#1e293b" : "transparent"

                Text {
                    anchors.left: parent.left
                    anchors.leftMargin: 12
                    anchors.verticalCenter: parent.verticalCenter
                    text: (modelData.kind === "directory" ? "▸  " : "   ") + modelData.name
                    color: "#e2e8f0"
                    font.pixelSize: 15
                }
                Text {
                    anchors.right: parent.right
                    anchors.rightMargin: 12
                    anchors.verticalCenter: parent.verticalCenter
                    text: modelData.kind === "file" ? modelData.size + " B" : ""
                    color: "#94a3b8"
                    font.pixelSize: 12
                }
                MouseArea {
                    id: rowMouse
                    anchors.fill: parent
                    hoverEnabled: true
                    cursorShape: Qt.PointingHandCursor
                    onClicked: filesystem.openEntry(modelData.name, modelData.kind)
                }
            }
        }

        Rectangle {
            id: createRow
            anchors.left: parent.left
            anchors.right: parent.right
            anchors.bottom: parent.bottom
            height: 54
            color: "#0f172a"

            Rectangle {
                anchors.left: parent.left
                anchors.leftMargin: 12
                anchors.verticalCenter: parent.verticalCenter
                width: 180
                height: 34
                radius: 6
                color: "#f8fafc"
                TextInput {
                    id: newName
                    anchors.fill: parent
                    anchors.margins: 8
                    verticalAlignment: TextInput.AlignVCenter
                    color: "#0f172a"
                    clip: true
                }
            }
            KButton {
                anchors.left: parent.left
                anchors.leftMargin: 202
                anchors.verticalCenter: parent.verticalCenter
                text: "+ File"
                onClicked: {
                    filesystem.createFile(newName.text)
                    newName.clear()
                }
            }
            KButton {
                anchors.right: parent.right
                anchors.rightMargin: 12
                anchors.verticalCenter: parent.verticalCenter
                text: "+ Dir"
                onClicked: {
                    filesystem.createDirectory(newName.text)
                    newName.clear()
                }
            }
        }
    }

    Rectangle {
        anchors.left: browser.right
        anchors.right: parent.right
        anchors.top: header.bottom
        anchors.bottom: status.top
        color: "#f8fafc"

        Text {
            id: selectedTitle
            anchors.left: parent.left
            anchors.right: parent.right
            anchors.top: parent.top
            anchors.margins: 18
            text: filesystem.selectedPath.length > 0
                  ? filesystem.selectedPath + "  ·  generation " + filesystem.generation
                  : "Select a text file"
            color: "#334155"
            font.pixelSize: 14
        }

        Rectangle {
            anchors.left: parent.left
            anchors.right: parent.right
            anchors.top: selectedTitle.bottom
            anchors.bottom: actions.top
            anchors.margins: 18
            color: "white"
            border.color: "#cbd5e1"
            radius: 6
            TextEdit {
                id: editor
                anchors.fill: parent
                anchors.margins: 12
                text: filesystem.content
                color: "#0f172a"
                font.family: "monospace"
                font.pixelSize: 14
                wrapMode: TextEdit.Wrap
                selectByMouse: true
            }
        }

        Row {
            id: actions
            anchors.right: parent.right
            anchors.rightMargin: 18
            anchors.bottom: parent.bottom
            anchors.bottomMargin: 14
            spacing: 8
            KButton { text: "Delete"; onClicked: filesystem.removeSelected() }
            KButton { text: "Save"; onClicked: filesystem.save(editor.text) }
        }
    }

    Rectangle {
        id: status
        anchors.left: parent.left
        anchors.right: parent.right
        anchors.bottom: parent.bottom
        height: 30
        color: "#020617"
        Text {
            anchors.left: parent.left
            anchors.leftMargin: 12
            anchors.verticalCenter: parent.verticalCenter
            text: filesystem.status
            color: "#94a3b8"
            font.pixelSize: 12
        }
    }
}
