import QtQuick

Rectangle {
    id: root
    property alias text: label.text
    signal clicked
    implicitWidth: label.implicitWidth + 24
    implicitHeight: 34
    radius: 6
    color: mouse.pressed ? "#334155" : "#1e293b"
    border.color: "#475569"

    Text {
        id: label
        anchors.centerIn: parent
        color: "#f8fafc"
        font.pixelSize: 13
    }

    MouseArea {
        id: mouse
        anchors.fill: parent
        cursorShape: Qt.PointingHandCursor
        onClicked: root.clicked()
    }
}
