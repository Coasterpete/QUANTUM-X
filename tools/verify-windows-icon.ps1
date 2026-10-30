param(
    [Parameter(Mandatory = $true)][string]$Executable,
    [string]$Icon = (Join-Path $PSScriptRoot '../assets/icons/quantum-dynamics/QUANTUM_Dynamics.ico')
)
$ErrorActionPreference = 'Stop'

# Inspect resources as data, without executing the PE or opening a desktop window.
# Keep this usable from Windows PowerShell 5.1; no Python or extra module required.
if (-not ('QuantumDynamics.IconVerifier' -as [type])) {
    Add-Type -TypeDefinition @'
using System;
using System.ComponentModel;
using System.IO;
using System.Runtime.InteropServices;

namespace QuantumDynamics {
    public static class IconVerifier {
        [DllImport("kernel32.dll", CharSet = CharSet.Unicode, SetLastError = true)]
        private static extern IntPtr LoadLibraryExW(string file, IntPtr reserved, uint flags);
        [DllImport("kernel32.dll", SetLastError = true)]
        private static extern bool FreeLibrary(IntPtr module);
        [DllImport("kernel32.dll", SetLastError = true)]
        private static extern IntPtr FindResourceW(IntPtr module, IntPtr name, IntPtr type);
        [DllImport("kernel32.dll", SetLastError = true)]
        private static extern uint SizeofResource(IntPtr module, IntPtr resource);
        [DllImport("kernel32.dll", SetLastError = true)]
        private static extern IntPtr LoadResource(IntPtr module, IntPtr resource);
        [DllImport("kernel32.dll")]
        private static extern IntPtr LockResource(IntPtr resource);
        [DllImport("user32.dll", SetLastError = true)]
        private static extern IntPtr LoadImageW(IntPtr module, IntPtr name, uint type,
            int width, int height, uint flags);
        [DllImport("user32.dll")]
        private static extern bool DestroyIcon(IntPtr icon);

        private static byte[] Resource(IntPtr module, int type, int id) {
            IntPtr found = FindResourceW(module, (IntPtr)id, (IntPtr)type);
            if (found == IntPtr.Zero)
                throw new Win32Exception(Marshal.GetLastWin32Error(),
                    "Missing icon resource type " + type + ", ID " + id);
            uint length = SizeofResource(module, found);
            IntPtr loaded = LoadResource(module, found);
            IntPtr data = LockResource(loaded);
            if (length == 0 || length > Int32.MaxValue || data == IntPtr.Zero)
                throw new InvalidDataException("Cannot read icon resource " + id);
            byte[] bytes = new byte[(int)length];
            Marshal.Copy(data, bytes, 0, bytes.Length);
            return bytes;
        }

        public static string[] Verify(string executable, string icon) {
            byte[] source = File.ReadAllBytes(icon);
            if (source.Length < 6 || BitConverter.ToUInt16(source, 0) != 0 ||
                BitConverter.ToUInt16(source, 2) != 1)
                throw new InvalidDataException("Expected a Windows ICO source file");
            int count = BitConverter.ToUInt16(source, 4);
            if (count == 0 || source.Length < 6 + count * 16)
                throw new InvalidDataException("Invalid ICO directory");
            // LOAD_LIBRARY_AS_DATAFILE | LOAD_LIBRARY_AS_IMAGE_RESOURCE: no code runs.
            IntPtr module = LoadLibraryExW(executable, IntPtr.Zero, 0x02 | 0x20);
            if (module == IntPtr.Zero)
                throw new Win32Exception(Marshal.GetLastWin32Error(), "Cannot inspect PE resources");
            try {
                const int groupId = 101; // apps/quantum-x/build.rs; qx-rendering::identity
                byte[] group = Resource(module, 14, groupId); // RT_GROUP_ICON
                if (group.Length != 6 + count * 14)
                    throw new InvalidDataException("Unexpected icon group length/image count");
                for (int b = 0; b < 6; b++)
                    if (source[b] != group[b])
                        throw new InvalidDataException("Embedded ICO header differs from source");
                string[] sizes = new string[count];
                for (int i = 0; i < count; i++) {
                    int src = 6 + i * 16, dst = 6 + i * 14;
                    // RC normalizes the PNG frames' unspecified plane count (0)
                    // to one plane. Permit that metadata-only normalization;
                    // every image payload below must still be byte-identical.
                    int sourcePlanes = BitConverter.ToUInt16(source, src + 4);
                    int resourcePlanes = BitConverter.ToUInt16(group, dst + 4);
                    if (resourcePlanes != sourcePlanes && !(sourcePlanes == 0 && resourcePlanes == 1))
                        throw new InvalidDataException("Icon frame plane count differs: " + i);
                    // The resource replaces the file offset with an RT_ICON ID.
                    for (int b = 0; b < 12; b++)
                        if (b != 4 && b != 5 && source[src + b] != group[dst + b])
                            throw new InvalidDataException("Icon frame " + i + " metadata byte " + b +
                                " differs: source=" + source[src + b] + ", resource=" + group[dst + b]);
                    uint length = BitConverter.ToUInt32(source, src + 8);
                    uint offset = BitConverter.ToUInt32(source, src + 12);
                    if (offset < 6 + count * 16 || (ulong)offset + length > (ulong)source.Length)
                        throw new InvalidDataException("ICO frame is out of bounds: " + i);
                    int id = BitConverter.ToUInt16(group, dst + 12);
                    byte[] image = Resource(module, 3, id); // RT_ICON
                    if ((uint)image.Length != length)
                        throw new InvalidDataException("Icon frame length differs: " + i);
                    for (int b = 0; b < image.Length; b++)
                        if (image[b] != source[(int)offset + b])
                            throw new InvalidDataException("Icon frame pixels differ: " + i);
                    sizes[i] = (source[src] == 0 ? 256 : source[src]) + "x" +
                        (source[src + 1] == 0 ? 256 : source[src + 1]);
                }
                // Exercise Windows' real icon decoder without creating a window.
                foreach (int size in new int[] { 32, 256 }) {
                    IntPtr decoded = LoadImageW(module, (IntPtr)groupId, 1, size, size, 0);
                    if (decoded == IntPtr.Zero)
                        throw new Win32Exception(Marshal.GetLastWin32Error(),
                            "Windows cannot load the embedded icon at size " + size);
                    DestroyIcon(decoded);
                }
                return sizes;
            } finally {
                FreeLibrary(module);
            }
        }
    }
}
'@
}

$executablePath = (Resolve-Path -LiteralPath $Executable).ProviderPath
$iconPath = (Resolve-Path -LiteralPath $Icon).ProviderPath
$sizes = [QuantumDynamics.IconVerifier]::Verify($executablePath, $iconPath)
[pscustomobject]@{
    executable = $executablePath
    resource_id = 101
    source_icon_sha256 = (Get-FileHash -LiteralPath $iconPath -Algorithm SHA256).Hash
    image_count = $sizes.Length
    sizes = @($sizes)
    native_decoder_sizes = @(32, 256)
    all_images_byte_identical = $true
}
