import QtQuick
import qs.Commons

Item {
  id: root

  property var overlay: null
  property color foreground: Color.menu.text

  readonly property bool show: {
    if (!overlay) return false
    if (overlay.offerHome && !overlay.currentView) return true
    if (overlay.error && !overlay.currentView) return true
    var view = overlay.currentView
    if (!overlay.scanning && view && (view.bytes === 0) && (!(view.list || []).length))
      return true
    return false
  }

  visible: show

  Column {
    anchors.centerIn: parent
    width: parent.width * 0.8
    spacing: Style.space(8)

    Text {
      width: parent.width
      text: overlay && overlay.error ? "!" : "○"
      color: overlay && overlay.error ? Color.urgent : root.foreground
      opacity: 0.8
      font.family: Style.font.menuFamily
      font.pixelSize: Style.font.displayLarge
      horizontalAlignment: Text.AlignHCenter
    }

    Text {
      width: parent.width
      wrapMode: Text.Wrap
      horizontalAlignment: Text.AlignHCenter
      color: overlay && overlay.error ? Color.urgent : root.foreground
      font.family: Style.font.menuFamily
      font.pixelSize: Style.font.title
      text: {
        if (!overlay) return ""
        if (overlay.error) return overlay.error
        return "Nothing here"
      }
    }

    Text {
      visible: overlay && overlay.offerHome
      width: parent.width
      horizontalAlignment: Text.AlignHCenter
      color: Color.accent
      font.family: Style.font.menuFamily
      font.pixelSize: Style.font.body
      text: "Open home directory"
      MouseArea {
        anchors.fill: parent
        cursorShape: Qt.PointingHandCursor
        onClicked: if (overlay) overlay.fallBackHome()
      }
    }
  }
}
