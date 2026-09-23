const fs = require('fs');

async function inspectFeatures() {
    const baseUrl = 'https://spatial.planning.vic.gov.au/server/rest/services/Hosted/AQ_North/SceneServer/layers/0';
    const geomRes = await fetch(`${baseUrl}/nodes/1/geometries/0`);
    const geomBuf = Buffer.from(await geomRes.arrayBuffer());
    const vertexCount = geomBuf.readUInt32LE(0);
    const featureCount = geomBuf.readUInt32LE(4);

    const posOffset = 8;
    const normOffset = posOffset + vertexCount * 12;
    const uvOffset = normOffset + vertexCount * 12;
    const colorOffset = uvOffset + vertexCount * 8;
    // After colorOffset + vertexCount * 4:
    let cursor = colorOffset + vertexCount * 4;
    console.log({ vertexCount, featureCount, cursor, totalLen: geomBuf.length });

    // Feature IDs
    const fids = [];
    for (let f = 0; f < featureCount; f++) {
        fids.push(geomBuf.readBigUInt64LE(cursor));
        cursor += 8;
    }
    const ranges = [];
    for (let f = 0; f < featureCount; f++) {
        const sf = geomBuf.readUInt32LE(cursor);
        const ef = geomBuf.readUInt32LE(cursor + 4);
        cursor += 8;
        ranges.push([sf, ef]);
    }
    console.log({ fids, ranges });

    // For each feature, check its vertex color range
    for (let f = 0; f < featureCount; f++) {
        const [sf, ef] = ranges[f];
        const startV = sf * 3;
        const endV = ef * 3 + 3;
        const fCols = {};
        for (let v = startV; v < endV; v++) {
            const co = colorOffset + v * 4;
            const col = `${geomBuf[co]},${geomBuf[co+1]},${geomBuf[co+2]}`;
            fCols[col] = (fCols[col] || 0) + 1;
        }
        console.log(`Feature ${fids[f]} (v ${startV}..${endV}):`, fCols);
    }
}

inspectFeatures().catch(console.error);
