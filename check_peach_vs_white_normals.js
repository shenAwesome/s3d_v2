const fs = require('fs');

async function test() {
    const baseUrl = 'https://spatial.planning.vic.gov.au/server/rest/services/Hosted/AQ_North/SceneServer/layers/0';
    const res = await fetch(`${baseUrl}/nodes/1/geometries/0`);
    const buf = Buffer.from(await res.arrayBuffer());
    const vCount = buf.readUInt32LE(0);
    const posOffset = 8;
    const normOffset = posOffset + vCount * 12;
    const colOffset = normOffset + vCount * 12 + vCount * 8;

    // Camera looking down from eye: say eye is at (0, 500, 500) looking towards (0, 0, 0)
    // V = eye - world_pos ≈ (0, 1, 1) normalized ≈ (0, 0.707, 0.707)
    // Local ENU: Y is Up, Z is -North. Camera is at pitch 50 looking down towards north.
    // So camera eye has Y > 0 and Z > 0.
    // V points towards camera: V.y > 0, V.z > 0.
    const V = [0, 0.707, 0.707];

    let discardedPeach = 0, keptPeach = 0;
    let discardedWhite = 0, keptWhite = 0;
    let discardedOrange = 0, keptOrange = 0;

    for (let t = 0; t < vCount / 3; t++) {
        const i = t * 3;
        const nx = buf.readFloatLE(normOffset + i * 12);
        const ny = buf.readFloatLE(normOffset + i * 12 + 4);
        const nz = buf.readFloatLE(normOffset + i * 12 + 8);
        // Engine normal: [nx, nz, -ny]
        const N = [nx, nz, -ny];
        const dot = N[0]*V[0] + N[1]*V[1] + N[2]*V[2];

        const r = buf[colOffset + i * 4];
        const g = buf[colOffset + i * 4 + 1];
        const b = buf[colOffset + i * 4 + 2];

        if (r === 255 && g === 226) { // Peach
            if (dot < 0) discardedPeach++; else keptPeach++;
        } else if (r === 255 && g === 255) { // White
            if (dot < 0) discardedWhite++; else keptWhite++;
        } else if (r === 252 && g === 197) { // Orange
            if (dot < 0) discardedOrange++; else keptOrange++;
        }
    }

    console.log({ discardedPeach, keptPeach, discardedWhite, keptWhite, discardedOrange, keptOrange });
}

test().catch(console.error);
