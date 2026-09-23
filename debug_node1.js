const fs = require('fs');

async function debugNode1() {
    const baseUrl = 'https://spatial.planning.vic.gov.au/server/rest/services/Hosted/AQ_North/SceneServer/layers/0';
    const geomRes = await fetch(`${baseUrl}/nodes/1/geometries/0`);
    const geomBuf = Buffer.from(await geomRes.arrayBuffer());
    const vertexCount = geomBuf.readUInt32LE(0);
    const posOffset = 8;
    const normOffset = posOffset + vertexCount * 12;
    const uvOffset = normOffset + vertexCount * 12;
    const colorOffset = uvOffset + vertexCount * 8;

    for (let i = 0; i < 20; i++) {
        const po = posOffset + i * 12;
        const no = normOffset + i * 12;
        const co = colorOffset + i * 4;
        const px = geomBuf.readFloatLE(po);
        const py = geomBuf.readFloatLE(po + 4);
        const pz = geomBuf.readFloatLE(po + 8);
        const nx = geomBuf.readFloatLE(no);
        const ny = geomBuf.readFloatLE(no + 4);
        const nz = geomBuf.readFloatLE(no + 8);
        const col = [geomBuf[co], geomBuf[co+1], geomBuf[co+2], geomBuf[co+3]];
        console.log(`v${i}: p=(${px.toFixed(2)}, ${py.toFixed(2)}, ${pz.toFixed(2)}) n=(${nx.toFixed(2)}, ${ny.toFixed(2)}, ${nz.toFixed(2)}) col=${col}`);
    }
}

debugNode1().catch(console.error);
