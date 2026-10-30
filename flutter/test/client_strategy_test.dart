import 'dart:convert';

import 'package:flutter_hbb/generated_bridge.dart';
import 'package:flutter_hbb/models/platform_model.dart';
import 'package:flutter_hbb/models/user_model.dart';
import 'package:flutter_test/flutter_test.dart';

class _Binding extends Fake implements RustdeskImpl {
  String? applied;
  String? appliedAt;
  Object? applyError;

  @override
  Future<void> mainApplyClientStrategy(
      {required String json, required String modifiedAt, dynamic hint}) async {
    applied = json;
    appliedAt = modifiedAt;
    if (applyError != null) throw applyError!;
  }
}

void main() {
  late _Binding binding;

  setUp(() {
    binding = _Binding();
    platformFFI.setBindingForTesting(binding);
  });

  test('passes a new controller strategy and its timestamp to Rust', () async {
    await UserModel.applyClientStrategy({
      'client_strategy': {
        'config_options': {
          'default-connect-password': 'abcd1234',
          'view_only': 'Y',
          'collapse_toolbar': '',
        }
      },
      'client_modified_at': 1729500000000,
    });

    expect(jsonDecode(binding.applied!), {
      'default-connect-password': 'abcd1234',
      'view_only': 'Y',
      'collapse_toolbar': '',
    });
    expect(binding.appliedAt, '1729500000000');
  });

  test('ignores responses from servers without controller strategy', () async {
    await UserModel.applyClientStrategy({'type': 'access_token'});

    expect(binding.applied, isNull);
  });

  test('a rejected strategy does not fail the login continuation', () async {
    binding.applyError = Exception('rejected');

    await UserModel.applyClientStrategy({
      'client_strategy': {
        'config_options': {'default-connect-password': 'abcd1234'}
      },
      'client_modified_at': 1729500000000,
    });

    expect(binding.applied, isNotNull);
  });
}
