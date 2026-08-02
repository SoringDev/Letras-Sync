import QtQuick

Rectangle {
    id: pill
    property alias text: pillText.text
    property color tint: "#2D3244"
    property color textColor: "#A0A4B8"
    radius: height / 2
    color: pill.tint
    implicitWidth: pillText.implicitWidth + 16
    implicitHeight: 22

    Text {
        id: pillText
        anchors.centerIn: parent
        color: pill.textColor
        font.family: "Inter"
        font.pixelSize: 11
        font.weight: Font.Medium
    }
}
