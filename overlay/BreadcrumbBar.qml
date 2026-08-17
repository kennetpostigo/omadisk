import QtQuick
import qs.Commons
import "OverlayModel.js" as Model

Item {
  id: root

  property var overlay: null
  property color foreground: Color.popups.text
  implicitHeight: Style.space(28)

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
    spacing: Style.space(2)

    Repeater {
      model: root.crumbs

      Item {
        required property var modelData
        required property int index
        height: parent.height
        width: crumbRow.implicitWidth + Style.space(10)

        readonly property bool current: index === root.crumbs.length - 1
        readonly property bool clickable: modelData.path !== "" && !current

        Rectangle {
          anchors.fill: parent
          radius: Style.cornerRadius
          color: crumbMouse.containsMouse && parent.clickable
            ? Util.alpha(root.foreground, 0.1)
            : "transparent"
        }

        Row {
          id: crumbRow
          anchors.centerIn: parent
          spacing: Style.space(6)

          Text {
            visible: index > 0
            text: "/"
            color: root.foreground
            opacity: 0.28
            font.family: Style.font.family
            font.pixelSize: Style.font.caption
            anchors.verticalCenter: parent.verticalCenter
          }

          Text {
            text: modelData.label
            color: root.foreground
            opacity: current ? 0.92 : (crumbMouse.containsMouse ? 0.85 : 0.5)
            font.family: Style.font.family
            font.pixelSize: Style.font.caption
            font.bold: current
            anchors.verticalCenter: parent.verticalCenter
          }
        }

        MouseArea {
          id: crumbMouse
          anchors.fill: parent
          hoverEnabled: true
          enabled: parent.clickable
          cursorShape: enabled ? Qt.PointingHandCursor : Qt.ArrowCursor
          onPressed: {
            if (overlay && modelData.path)
              overlay.drill(modelData.path)
          }
        }
      }
    }
  }
}
