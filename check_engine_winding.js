const fs = require('fs');

async function checkEngineWinding() {
    const baseUrl = 'https://spatial.planning.vic.gov.au/server/rest/services/Hosted/AQ_North/SceneServer/layers/0';
    const geomRes = await fetch(`${baseUrl}/nodes/1/geometries/0`);
    const geomBuf = Buffer.from(await geomRes.arrayBuffer());
    const vertexCount = geomBuf.readUInt32LE(0);
    const posOffset = 8;
    const normOffset = posOffset + vertexCount * 12;
    const colorOffset = normOffset + vertexCount * 12 + vertexCount * 8;

    for (let t of [2, 3, 4, 100, 101, 102]) {
        const v0 = t * 3;
        const v1 = t * 3 + 1;
        const v2 = t * 3 + 2;

        const getP = (v) => {
            const px = geomBuf.readFloatLE(posOffset + v*12);
            const py = geomBuf.readFloatLE(posOffset + v*12 + 4);
            const pz = geomBuf.readFloatLE(posOffset + v*12 + 8);
            // In decoder.rs: px is East offset (meters), py is North offset (meters), pz is Up offset (meters)
            // Local ENU: x = px, y = pz (Up), z = -py (South = -North)
            return [px, pz, -py];
        };
        const getN = (v) => {
            const nx = geomBuf.readFloatLE(normOffset + v*12);
            const ny = geomBuf.readFloatLE(normOffset + v*12 + 4);
            const nz = geomBuf.readFloatLE(normOffset + v*12 + 8);
            return [nx, nz, -ny];
        };
        const getC = (v) => [geomBuf[colorOffset + v*4], geomBuf[colorOffset + v*4 + 1], geomBuf[colorOffset + v*4 + 2]];

        const p0 = getP(v0), p1 = getP(v1), p2 = getP(v2);
        const e1 = [p1[0] - p0[0], p1[1] - p0[1], p1[2] - p0[2]];
        const e2 = [p2[0] - p0[0], p2[1] - p0[1], p2[2] - p0[2]];
        const cross = [
            e1[1] * e2[2] - e1[2] * e2[1],
            e1[2] * e2[0] - e1[0] * e2[2],
            e1[0] * e2[1] - e1[1] * e2[0]
        ];
        const n = getN(v0);
        const dot = cross[0] * n[0] + cross[1] * n[1] + cross[2] * n[2];
        console.log(`Triangle ${t} (color ${getC(v0)}): cross dot normal = ${dot.toFixed(2)} (${dot > 0 ? 'CCW' : 'CW'})`);
    }
}

checkEngineWinding().catch(console.error);
