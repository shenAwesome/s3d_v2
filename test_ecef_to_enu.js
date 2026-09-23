const lat_deg = -37.774764960689154;
const lon_deg = 144.82791511530388;

const lat_rad = lat_deg * Math.PI / 180;
const lon_rad = lon_deg * Math.PI / 180;

const sin_lat = Math.sin(lat_rad);
const cos_lat = Math.cos(lat_rad);
const sin_lon = Math.sin(lon_rad);
const cos_lon = Math.cos(lon_rad);

// Earth up in ECEF:
const expected_ecef_up = [cos_lat * cos_lon, cos_lat * sin_lon, sin_lat];
console.log('Expected ECEF Up at Melbourne:', expected_ecef_up);

// Node 1 v0 normal from buffer:
const v0_norm = [-0.6461150646209717, 0.45531120896339417, -0.6125577688217163];
console.log('Node 1 v0 norm in buffer:     ', v0_norm);

// Convert ECEF vector to ENU:
function ecefToEnu(dx, dy, dz) {
    const east = -sin_lon * dx + cos_lon * dy;
    const north = -sin_lat * cos_lon * dx - sin_lat * sin_lon * dy + cos_lat * dz;
    const up = cos_lat * cos_lon * dx + cos_lat * sin_lon * dy + sin_lat * dz;
    return { east, north, up };
}

const enu = ecefToEnu(v0_norm[0], v0_norm[1], v0_norm[2]);
console.log('Converted to ENU:', enu);
console.log('Engine space [East, Up, -North]:', [enu.east, enu.up, -enu.north]);
