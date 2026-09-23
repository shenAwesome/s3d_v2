const fs = require('fs');

async function checkFeatures() {
    const baseUrl = 'https://spatial.planning.vic.gov.au/server/rest/services/Hosted/AQ_North/SceneServer/layers/0';
    const res = await fetch(`${baseUrl}/nodes/1/geometries/0`);
    const buf = Buffer.from(await res.arrayBuffer());
    const vCount = buf.readUInt32LE(0);
    const fCount = buf.readUInt32LE(4);
    const posOffset = 8;
    const normOffset = posOffset + vCount * 12;
    const uvOffset = normOffset + vCount * 12;
    const colOffset = uvOffset + vCount * 8;
    const featOffset = colOffset + vCount * 4;

    console.log({ vCount, fCount, featOffset, bufLen: buf.length });

    // Read features
    let cur = featOffset;
    const featIds = [];
    for (let f = 0; f < fCount; f++) {
        featIds.push(buf.readBigUInt64LE(cur));
        cur += 8;
    }
    const faceRanges = [];
    for (let f = 0; f < fCount; f++) {
        faceRanges.push([buf.readUInt32LE(cur), buf.readUInt32LE(cur + 4)]);
        cur += 8;
    }
    console.log({ featIds, faceRanges });

    // What colors are in feature 0 vs feature 1?
    for (let f = 0; f < fCount; f++) {
        const [startFace, endFace] = faceRanges[f];
        const colors = new Set();
        for (let face = startFace; face <= endFace; face++) {
            const v = face * 3;
            const c = `${buf[colOffset + v*4]},${buf[colOffset + v*4 + 1]},${buf[colOffset + v*4 + 2]}`;
            colors.add(c);
        }
        console.log(`Feature ${f} (id=${featIds[f]}): faces ${startFace}..${endFace}, colors=`, Array.from(colors));
    }
}

checkFeatures().catch(console.error);
