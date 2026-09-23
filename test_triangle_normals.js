const fs = require('fs');

async function testNormals() {
    const baseUrl = 'https://spatial.planning.vic.gov.au/server/rest/services/Hosted/AQ_North/SceneServer/layers/0';
    const geomRes = await fetch(`${baseUrl}/nodes/1/geometries/0`);
    const geomBuf = Buffer.from(await geomRes.arrayBuffer());
    const vertexCount = geomBuf.readUInt32LE(0);
    const posOffset = 8;
    const normOffset = posOffset + vertexCount * 12;
    const colorOffset = normOffset + vertexCount * 12 + vertexCount * 8;

    // Node 1 OBB center
    // Let's get node 1 JSON
    const nodeRes = await fetch(`${baseUrl}/nodes/1`);
    const nodeJson = await nodeRes.json();
    console.log('Node 1 mbs:', nodeJson.mbs, 'obb:', nodeJson.obb);

    // Check triangle vertices for triangle with peach color (v6, v7, v8)
    for (let t = 2; t < 6; t++) {
        const v = [t * 3, t * 3 + 1, t * 3 + 2];
        const pts = v.map(idx => {
            const px = geomBuf.readFloatLE(posOffset + idx * 12);
            const py = geomBuf.readFloatLE(posOffset + idx * 12 + 4);
            const pz = geomBuf.readFloatLE(posOffset + idx * 12 + 8);
            // Engine space: x = px, y = pz (Up), z = -py (-North)
            return [px, pz, -py];
        });
        const norms = v.map(idx => {
            const nx = geomBuf.readFloatLE(normOffset + idx * 12);
            const ny = geomBuf.readFloatLE(normOffset + idx * 12 + 4);
            const nz = geomBuf.readFloatLE(normOffset + idx * 12 + 8);
            return [nx, nz, -ny];
        });
        const col = [geomBuf[colorOffset + v[0] * 4], geomBuf[colorOffset + v[0] * 4 + 1], geomBuf[colorOffset + v[0] * 4 + 2]];

        const e1 = [pts[1][0] - pts[0][0], pts[1][1] - pts[0][1], pts[1][2] - pts[0][2]];
        const e2 = [pts[2][0] - pts[0][0], pts[2][1] - pts[0][1], pts[2][2] - pts[0][2]];
        const cross = [
            e1[1] * e2[2] - e1[2] * e2[1],
            e1[2] * e2[0] - e1[0] * e2[2],
            e1[0] * e2[1] - e1[1] * e2[0]
        ];
        const len = Math.hypot(...cross);
        if (len > 1e-6) {
            cross[0] /= len; cross[1] /= len; cross[2] /= len;
        }

        const dot = cross[0] * norms[0][0] + cross[1] * norms[0][1] + cross[2] * norms[0][2];
        console.log(`Triangle ${t} col=${col}: pts=`, pts);
        console.log(`  e1=`, e1, `e2=`, e2);
        console.log(`  cross=`, cross, `norm=`, norms[0], `dot=${dot}`);
    }
}

testNormals().catch(console.error);
