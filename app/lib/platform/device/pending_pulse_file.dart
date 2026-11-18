import 'dart:io';
import 'dart:typed_data';

import '../pulsys_api.dart';

final class PendingPulseFile {
  PendingPulseFile(Directory directory)
    : _file = File('${directory.path}${Platform.pathSeparator}pulse-pending');

  final File _file;
  static const _unavailable = ApiError(
    code: ApiErrorCodes.storageError,
    message: 'The pending pulse file is unavailable.',
  );

  Future<Uint8List?> read() async {
    try {
      return await _file.readAsBytes();
    } on PathNotFoundException {
      return null;
    } on FileSystemException {
      throw _unavailable;
    }
  }

  Future<void> stage(Uint8List bytes) async {
    try {
      final temporary = File('${_file.path}.tmp');
      await temporary.writeAsBytes(bytes, flush: true);
      await temporary.rename(_file.path);
    } on FileSystemException {
      throw _unavailable;
    }
  }

  Future<void> clear() async {
    try {
      await _file.delete();
    } on PathNotFoundException {
      return;
    } on FileSystemException {
      throw _unavailable;
    }
  }
}
