import QtQuick
import QtQuick.Controls

Button {
    id: appBtn
    property string variant: "secondary" // primary | secondary | danger | ghost
    property int textPixelSize: 13
    hoverEnabled: true
    padding: 10
    leftPadding: 16
    rightPadding: 16

    background: Rectangle {
        radius: 8
        color: {
            if (!appBtn.enabled) return "#1A1B20"
            if (appBtn.variant === "primary")
                return appBtn.down ? "#2563EB" : (appBtn.hovered ? "#4C8DF7" : "#3B82F6")
            if (appBtn.variant === "danger")
                return appBtn.down ? "#3A1418" : (appBtn.hovered ? "#2A1519" : "#201318")
            if (appBtn.variant === "ghost")
                return appBtn.down ? "#22252E" : (appBtn.hovered ? "#1C1F29" : "transparent")
            return appBtn.down ? "#2D3244" : (appBtn.hovered ? "#242935" : "#1E1E24")
        }
        border.width: appBtn.variant === "danger" || appBtn.variant === "secondary" ? 1 : 0
        border.color: appBtn.variant === "danger" ? "#DC2626" : "#2E303C"
    }
    contentItem: Text {
        text: appBtn.text
        color: {
            if (!appBtn.enabled) return "#5A5E70"
            if (appBtn.variant === "danger") return "#F87171"
            if (appBtn.variant === "ghost") return appBtn.hovered ? "#FFFFFF" : "#A0A4B8"
            return "#FFFFFF"
        }
        font.family: "Inter"
        font.pixelSize: appBtn.textPixelSize
        font.weight: Font.Medium
        horizontalAlignment: Text.AlignHCenter
        verticalAlignment: Text.AlignVCenter
        elide: Text.ElideRight
    }
}
