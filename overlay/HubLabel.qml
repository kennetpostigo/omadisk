import QtQuick
import qs.Commons
import "Format.js" as Format
import "OverlayModel.js" as Model

Item {
  id: root

  property var overlay: null
  property color foreground: Color.popups.text

  readonly property var view: overlay ? overlay.currentView : null
  readonly property var hovered: {
    if (!overlay) return null
    void overlay.hoverTick
    if (!overlay.hoverPath) return null
    if (overlay.hoverSlice) return overlay.hoverSlice
    var rows = overlay.listRows || []
    for (var i = 0; i < rows.length; i++) {
      if (rows[i].path === overlay.hoverPath)
        return { name: rows[i].name, bytes: rows[i].bytes }
    }
    if (view && view.path === overlay.hoverPath)
      return { name: view.name, bytes: view.bytes }
    return null
  }

  Column {
    anchors.centerIn: parent
    width: parent.width * 0.84
    spacing: Style.space(2)

    Text {
      width: parent.width
      text: {
        if (root.hovered) return Format.bytes(root.hovered.bytes)
        if (root.view) return Format.bytes(root.view.bytes)
        return overlay && overlay.scanning ? "…" : ""
      }
      color: root.foreground
      font.family: Style.font.family
      font.pixelSize: root.width < Style.space(88) ? Style.font.body : Style.font.title
      font.bold: true
      horizontalAlignment: Text.AlignHCenter
      elide: Text.ElideRight
    }

    Text {
      width: parent.width
      text: {
        if (root.hovered) return root.hovered.name || Model.basename(overlay.hoverPath)
        if (!overlay) return ""
        if (overlay.scanning && (!root.view || root.view.bytes === undefined)) return "scanning"
        if (!root.view) return ""
        return root.view.name || ""
      }
      color: root.foreground
      opacity: 0.5
      font.family: Style.font.family
      font.pixelSize: Style.font.caption
      horizontalAlignment: Text.AlignHCenter
      elide: Text.ElideMiddle
    }
  }
}
