const fs = require('fs');

async function inspectTriangles() {
    const baseUrl = 'https://spatial.planning.vic.gov.au/server/rest/services/Hosted/AQ_North/SceneServer/layers/0';
    const geomRes = await fetch(`${baseUrl}/nodes/1/geometries/0`);
    const geomBuf = Buffer.from(await geomRes.arrayBuffer());
    const vertexCount = geomBuf.readUInt32LE(0);
    const posOffset = 8;
    const normOffset = posOffset + vertexCount * 12;
    const uvOffset = normOffset + vertexCount * 12;
    const colorOffset = uvOffset + vertexCount * 8;

    let roofPeach = 0, roofWhite = 0, wallPeach = 0, wallWhite = 0;
    for (let i = 0; i < vertexCount; i++) {
        const no = normOffset + i * 12;
        const nx = geomBuf.readFloatLE(no);
        const ny = geomBuf.readFloatLE(no + 4);
        const nz = geomBuf.readFloatLE(no + 8);
        const co = colorOffset + i * 4;
        const col = `${geomBuf[co]},${geomBuf[co+1]},${geomBuf[co+2]}`;

        // ny or nz pointing up? Remember normalReferenceFrame is earth-centered or ENU
        const isUp = Math.abs(nz) > 0.7 || Math.abs(ny) > 0.7;
        if (col === '255,226,165') {
            if (isUp) roofPeach++; else wallPeach++;
        } else if (col === '255,255,255') {
            if (isUp) roofWhite++; else wallWhite++;
        }
    }
    console.log({ roofPeach, roofWhite, wallPeach, wallWhite });
}

inspectTriangles().catch(console.error);
