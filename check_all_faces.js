const fs = require('fs');

async function checkAllFaces() {
    const baseUrl = 'https://spatial.planning.vic.gov.au/server/rest/services/Hosted/AQ_North/SceneServer/layers/0';
    const geomRes = await fetch(`${baseUrl}/nodes/1/geometries/0`);
    const geomBuf = Buffer.from(await geomRes.arrayBuffer());
    const vertexCount = geomBuf.readUInt32LE(0);
    const featureCount = geomBuf.readUInt32LE(4);
    const posOffset = 8;
    const normOffset = posOffset + vertexCount * 12;
    const colorOffset = normOffset + vertexCount * 12 + vertexCount * 8;

    console.log(`Node 1: vertexCount=${vertexCount}, triangles=${vertexCount/3}, features=${featureCount}`);

    // Count triangles by color
    const colorCounts = {};
    for (let i = 0; i < vertexCount; i += 3) {
        const c0 = `${geomBuf[colorOffset + i*4]},${geomBuf[colorOffset + i*4 + 1]},${geomBuf[colorOffset + i*4 + 2]}`;
        colorCounts[c0] = (colorCounts[c0] || 0) + 1;
    }
    console.log('Triangles per color in node 1:', colorCounts);

    // Look at the indices / vertices of peach vs white vs orange
    for (let t = 0; t < vertexCount / 3; t++) {
        const i = t * 3;
        const c = `${geomBuf[colorOffset + i*4]},${geomBuf[colorOffset + i*4 + 1]},${geomBuf[colorOffset + i*4 + 2]}`;
        const p0 = [geomBuf.readFloatLE(posOffset + i*12), geomBuf.readFloatLE(posOffset + i*12 + 4), geomBuf.readFloatLE(posOffset + i*12 + 8)];
        const n0 = [geomBuf.readFloatLE(normOffset + i*12), geomBuf.readFloatLE(normOffset + i*12 + 4), geomBuf.readFloatLE(normOffset + i*12 + 8)];
        if (t < 15 || t > vertexCount/3 - 5) {
            console.log(`Triangle ${t} col=${c} z=${p0[2].toFixed(2)} n=(${n0[0].toFixed(2)}, ${n0[1].toFixed(2)}, ${n0[2].toFixed(2)})`);
        }
    }
}

checkAllFaces().catch(console.error);
