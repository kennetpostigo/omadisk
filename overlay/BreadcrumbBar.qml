import QtQuick
import qs.Commons
import "OverlayModel.js" as Model

Item {
  id: root

  property var overlay: null
  property color foreground: Color.popups.text
  implicitHeight: Style.space(22)

  readonly property var crumbs: {
    if (!overlay || !overlay.scanRoot) return []
    var rootPath = overlay.scanRoot
    var focus = overlay.focusPath || rootPath
    var home = overlay.homeDir()
    var parts = []
    parts.push({
      path: rootPath,
      label: rootPath === home ? "~" : Model.basename(rootPath)
    })
    if (focus !== rootPath && Model.isDescendant(rootPath, focus)) {
      var rest = focus.substring(rootPath.length).replace(/^\//, "")
      var segs = rest.split("/").filter(function(s) { return s.length > 0 })
      var acc = rootPath
      for (var i = 0; i < segs.length; i++) {
        acc = Model.joinUnder(acc, segs[i])
        parts.push({ path: acc, label: segs[i] })
      }
    }
    if (parts.length > 4)
      return [parts[0], { path: "", label: "…" }].concat(parts.slice(parts.length - 2))
    return parts
  }

  Row {
    anchors.fill: parent
    spacing: Style.space(4)

    Repeater {
      model: root.crumbs

      Text {
        required property var modelData
        required property int index
        text: (index > 0 ? " / " : "") + modelData.label
        color: root.foreground
        opacity: index === root.crumbs.length - 1 ? 0.92 : 0.45
        font.family: Style.font.family
        font.pixelSize: Style.font.caption
        font.bold: index === root.crumbs.length - 1
        anchors.verticalCenter: parent.verticalCenter

        MouseArea {
          anchors.fill: parent
          enabled: modelData.path !== ""
          cursorShape: enabled ? Qt.PointingHandCursor : Qt.ArrowCursor
          onClicked: if (overlay && modelData.path) overlay.drill(modelData.path)
        }
      }
    }
  }
}
