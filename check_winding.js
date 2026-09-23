const fs = require('fs');

async function checkWinding() {
    const baseUrl = 'https://spatial.planning.vic.gov.au/server/rest/services/Hosted/AQ_North/SceneServer/layers/0';
    const geomRes = await fetch(`${baseUrl}/nodes/1/geometries/0`);
    const geomBuf = Buffer.from(await geomRes.arrayBuffer());
    const vertexCount = geomBuf.readUInt32LE(0);
    const posOffset = 8;
    const normOffset = posOffset + vertexCount * 12;
    const uvOffset = normOffset + vertexCount * 12;
    const colorOffset = uvOffset + vertexCount * 8;

    // Check triangle 2 (v6, v7, v8) which is peach
    // Let's compute geometric normal via cross product: (p1 - p0) x (p2 - p0)
    for (let t = 2; t < 5; t++) {
        const v0 = t * 3;
        const v1 = t * 3 + 1;
        const v2 = t * 3 + 2;

        const getP = (v) => [geomBuf.readFloatLE(posOffset + v*12), geomBuf.readFloatLE(posOffset + v*12 + 4), geomBuf.readFloatLE(posOffset + v*12 + 8)];
        const getN = (v) => [geomBuf.readFloatLE(normOffset + v*12), geomBuf.readFloatLE(normOffset + v*12 + 4), geomBuf.readFloatLE(normOffset + v*12 + 8)];
        const getC = (v) => [geomBuf[colorOffset + v*4], geomBuf[colorOffset + v*4 + 1], geomBuf[colorOffset + v*4 + 2]];

        const p0 = getP(v0), p1 = getP(v1), p2 = getP(v2);
        const e1 = [p1[0] - p0[0], p1[1] - p0[1], p1[2] - p0[2]];
        const e2 = [p2[0] - p0[0], p2[1] - p0[1], p2[2] - p0[2]];
        // Cross product e1 x e2
        const cross = [
            e1[1] * e2[2] - e1[2] * e2[1],
            e1[2] * e2[0] - e1[0] * e2[2],
            e1[0] * e2[1] - e1[1] * e2[0]
        ];
        const n = getN(v0);
        const dot = cross[0] * n[0] + cross[1] * n[1] + cross[2] * n[2];
        console.log(`Triangle ${t} (color ${getC(v0)}): dot(cross, normal) = ${dot.toFixed(4)} -> ${dot > 0 ? 'CCW' : 'CW'}`);
    }

    // Also check triangle around 100 which is white
    for (let t = 100; t < 103; t++) {
        const v0 = t * 3;
        const v1 = t * 3 + 1;
        const v2 = t * 3 + 2;
        const getP = (v) => [geomBuf.readFloatLE(posOffset + v*12), geomBuf.readFloatLE(posOffset + v*12 + 4), geomBuf.readFloatLE(posOffset + v*12 + 8)];
        const getN = (v) => [geomBuf.readFloatLE(normOffset + v*12), geomBuf.readFloatLE(normOffset + v*12 + 4), geomBuf.readFloatLE(normOffset + v*12 + 8)];
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
        console.log(`Triangle ${t} (color ${getC(v0)}): dot(cross, normal) = ${dot.toFixed(4)} -> ${dot > 0 ? 'CCW' : 'CW'}`);
    }
}

checkWinding().catch(console.error);
