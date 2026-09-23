const fs = require('fs');

async function checkNode() {
    const baseUrl = 'https://spatial.planning.vic.gov.au/server/rest/services/Hosted/AQ_North/SceneServer/layers/0';
    // Let's fetch geometry 0 of node 1
    const geomRes = await fetch(baseUrl + '/nodes/1/geometries/0');
    const geomBuf = Buffer.from(await geomRes.arrayBuffer());
    console.log('Geometry buffer length:', geomBuf.length);

    // Read header: vertexCount, featureCount
    const vertexCount = geomBuf.readUInt32LE(0);
    const featureCount = geomBuf.readUInt32LE(4);
    console.log({ vertexCount, featureCount });

    // 8 + 804 * 12 (pos) = 8 + 9648 = 9656
    // 9656 + 804 * 12 (norm) = 9656 + 9648 = 19304
    // 19304 + 804 * 8 (uv) = 19304 + 6432 = 25736
    // 25736 + 804 * 4 (color) = 25736 + 3216 = 28952
    const posOffset = 8;
    const normOffset = posOffset + vertexCount * 12;
    const uvOffset = normOffset + vertexCount * 12;
    const colorOffset = uvOffset + vertexCount * 8;

    const colors = [];
    for (let i = 0; i < Math.min(vertexCount, 20); i++) {
        const o = colorOffset + i * 4;
        colors.push([geomBuf[o], geomBuf[o+1], geomBuf[o+2], geomBuf[o+3]]);
    }
    console.log('Sample vertex colors:', colors);

    // Let's check unique colors across all vertices
    const uniqueColors = new Set();
    for (let i = 0; i < vertexCount; i++) {
        const o = colorOffset + i * 4;
        uniqueColors.add(`${geomBuf[o]},${geomBuf[o+1]},${geomBuf[o+2]},${geomBuf[o+3]}`);
    }
    console.log('All unique vertex colors in node 1:', Array.from(uniqueColors));

    // Also check attributes
    const attrRes = await fetch(baseUrl + '/nodes/1/attributes/f_3/0');
    console.log('Attribute f_3 status:', attrRes.status);
    if (attrRes.ok) {
        const attrBuf = Buffer.from(await attrRes.arrayBuffer());
        console.log('Attribute f_3 buf length:', attrBuf.length);
        console.log('Attribute f_3 as text/buffer:', attrBuf.toString('utf8'));
    }
}

checkNode().catch(console.error);
