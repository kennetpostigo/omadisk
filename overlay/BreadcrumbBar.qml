import QtQuick
import qs.Commons
import "OverlayModel.js" as Model

Item {
  id: root

  property var overlay: null
  property color foreground: Color.menu.text
  property int rowHeight: Style.space(28)

  implicitHeight: rowHeight

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
    if (parts.length > 5) {
      return [parts[0], { path: "", label: "…" }].concat(parts.slice(parts.length - 2))
    }
    return parts
  }

  Row {
    id: row
    anchors.fill: parent
    spacing: Style.space(6)

    Repeater {
      model: root.crumbs

      Row {
        required property var modelData
        required property int index
        spacing: Style.space(6)

        Text {
          visible: index > 0
          text: "/"
          color: root.foreground
          opacity: 0.4
          font.family: Style.font.menuFamily
          font.pixelSize: Style.font.body
          anchors.verticalCenter: parent.verticalCenter
        }

        Text {
          text: modelData.label
          color: index === root.crumbs.length - 1 ? root.foreground : root.foreground
          opacity: index === root.crumbs.length - 1 ? 1 : 0.7
          font.family: Style.font.menuFamily
          font.pixelSize: Style.font.body
          font.bold: index === root.crumbs.length - 1
          anchors.verticalCenter: parent.verticalCenter

          MouseArea {
            anchors.fill: parent
            enabled: modelData.path !== ""
            cursorShape: enabled ? Qt.PointingHandCursor : Qt.ArrowCursor
            onClicked: if (overlay) overlay.drill(modelData.path)
          }
        }
      }
    }
  }
}
