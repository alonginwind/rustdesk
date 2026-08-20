import 'dart:convert';

import 'package:desktop_multi_window/desktop_multi_window.dart';
import 'package:flutter/material.dart';
import 'package:flutter_hbb/common.dart';
import 'package:flutter_hbb/common/shared_state.dart';
import 'package:flutter_hbb/common/widgets/dialog.dart';
import 'package:flutter_hbb/consts.dart';
import 'package:flutter_hbb/models/state_model.dart';
import 'package:flutter_hbb/desktop/pages/file_manager_page.dart';
import 'package:flutter_hbb/desktop/widgets/tabbar_widget.dart';
import 'package:flutter_hbb/utils/multi_window_manager.dart';
import 'package:flutter_svg/flutter_svg.dart';
import 'package:get/get.dart';

import '../../models/platform_model.dart';

/// File Transfer for multi tabs
class FileManagerTabPage extends StatefulWidget {
  final Map<String, dynamic> params;

  const FileManagerTabPage({Key? key, required this.params}) : super(key: key);

  @override
  State<FileManagerTabPage> createState() => _FileManagerTabPageState(params);
}

class _FileManagerTabPageState extends State<FileManagerTabPage> {
  DesktopTabController get tabController => Get.find<DesktopTabController>();

  static const IconData selectedIcon = Icons.file_copy_sharp;
  static const IconData unselectedIcon = Icons.file_copy_outlined;

  _FileManagerTabPageState(Map<String, dynamic> params) {
    Get.put(DesktopTabController(tabType: DesktopTabType.fileTransfer));
    tabController.onSelected = (id) {
      WindowController.fromWindowId(windowId())
          .setTitle(getWindowNameWithId(id));
    };
    tabController.onRemoved = (_, id) {
      ConnectionTypeState.delete(id);
      FingerprintState.delete(id);
      onRemoveId(id);
    };
    final initialId = params['id']!;
    ConnectionTypeState.init(initialId);
    FingerprintState.init(initialId);
    tabController.add(TabInfo(
        key: initialId,
        label: initialId,
        selectedIcon: selectedIcon,
        unselectedIcon: unselectedIcon,
        onTabCloseButton: () async {
          if (await desktopTryShowTabAuditDialogCloseCancelled(
            id: initialId,
            tabController: tabController,
          )) {
            return;
          }
          tabController.closeBy(initialId);
        },
        page: FileManagerPage(
          key: ValueKey(initialId),
          id: initialId,
          password: params['password'],
          isSharedPassword: params['isSharedPassword'],
          tabController: tabController,
          forceRelay: params['forceRelay'],
          connToken: params['connToken'],
        )));
  }

  @override
  void initState() {
    super.initState();

    rustDeskWinManager.setMethodHandler((call, fromWindowId) async {
      debugPrint(
          "[FileTransfer] call ${call.method} with args ${call.arguments} from window $fromWindowId to ${windowId()}");
      // for simplify, just replace connectionId
      if (call.method == kWindowEventNewFileTransfer) {
        final args = jsonDecode(call.arguments);
        final id = args['id'];
        ConnectionTypeState.init(id);
        FingerprintState.init(id);
        windowOnTop(windowId());
        tabController.add(TabInfo(
            key: id,
            label: id,
            selectedIcon: selectedIcon,
            unselectedIcon: unselectedIcon,
            onTabCloseButton: () async {
              if (await desktopTryShowTabAuditDialogCloseCancelled(
                id: id,
                tabController: tabController,
              )) {
                return;
              }
              tabController.closeBy(id);
            },
            page: FileManagerPage(
              key: ValueKey(id),
              id: id,
              password: args['password'],
              isSharedPassword: args['isSharedPassword'],
              tabController: tabController,
              forceRelay: args['forceRelay'],
              connToken: args['connToken'],
            )));
      } else if (call.method == "onDestroy") {
        tabController.clear();
      } else if (call.method == kWindowActionRebuild) {
        reloadCurrentWindow();
      }
    });
    Future.delayed(Duration.zero, () {
      restoreWindowPosition(WindowType.FileTransfer, windowId: windowId());
    });
  }

  @override
  Widget build(BuildContext context) {
    final child = Scaffold(
        backgroundColor: Theme.of(context).cardColor,
        body: DesktopTab(
          controller: tabController,
          onWindowCloseButton: handleWindowCloseButton,
          tail: const AddButton(),
          selectedBorderColor: MyTheme.accent,
          labelGetter: DesktopTab.tablabelGetter,
          tabBuilder: (key, icon, label, themeConf) => Obx(() {
            final connectionType = ConnectionTypeState.find(key);
            if (!connectionType.isValid()) {
              return Row(
                mainAxisAlignment: MainAxisAlignment.center,
                children: [
                  icon,
                  label,
                ],
              );
            } else {
              bool secure =
                  connectionType.secure.value == ConnectionType.strSecure;
              bool direct =
                  connectionType.direct.value == ConnectionType.strDirect;
              String msgConn = getConnectionText(
                  secure, direct, connectionType.stream_type.value);
              var msgFingerprint = '${translate('Fingerprint')}:\n';
              var fingerprint = FingerprintState.find(key).value;
              if (fingerprint.isEmpty) {
                fingerprint = 'N/A';
              }
              if (fingerprint.length > 5 * 8) {
                var first = fingerprint.substring(0, 39);
                var second = fingerprint.substring(40);
                msgFingerprint += '$first\n$second';
              } else {
                msgFingerprint += fingerprint;
              }
              return Row(
                mainAxisAlignment: MainAxisAlignment.center,
                children: [
                  icon,
                  Tooltip(
                    message: '$msgConn\n$msgFingerprint',
                    child: SvgPicture.asset(
                      'assets/${connectionType.secure.value}${connectionType.direct.value}.svg',
                      width: themeConf.iconSize,
                      height: themeConf.iconSize,
                    ).paddingOnly(right: 5),
                  ),
                  label,
                ],
              );
            }
          }),
        ));
    final tabWidget = isLinux
        ? buildVirtualWindowFrame(context, child)
        : workaroundWindowBorder(
            context,
            Container(
              decoration: BoxDecoration(
                  border: Border.all(color: MyTheme.color(context).border!)),
              child: child,
            ));
    return isMacOS || kUseCompatibleUiMode
        ? tabWidget
        : SubWindowDragToResizeArea(
            child: tabWidget,
            resizeEdgeSize: stateGlobal.resizeEdgeSize.value,
            enableResizeEdges: subWindowManagerEnableResizeEdges,
            windowId: stateGlobal.windowId,
          );
  }

  void onRemoveId(String id) {
    if (tabController.state.value.tabs.isEmpty) {
      WindowController.fromWindowId(windowId()).close();
    }
  }

  int windowId() {
    return widget.params["windowId"];
  }

  Future<bool> handleWindowCloseButton() async {
    final connLength = tabController.state.value.tabs.length;
    if (connLength == 1) {
      if (await desktopTryShowTabAuditDialogCloseCancelled(
        id: tabController.state.value.tabs[0].key,
        tabController: tabController,
      )) {
        return false;
      }
    }
    if (connLength <= 1) {
      tabController.clear();
      return true;
    } else {
      final bool res;
      if (!option2bool(kOptionEnableConfirmClosingTabs,
          bind.mainGetLocalOption(key: kOptionEnableConfirmClosingTabs))) {
        res = true;
      } else {
        res = await closeConfirmDialog();
      }
      if (res) {
        tabController.clear();
      }
      return res;
    }
  }
}
