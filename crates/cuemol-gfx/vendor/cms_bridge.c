#include "lcms2.h"
#include <stdlib.h>
#include <string.h>

struct CueMolCms {
    cmsContext context;
    cmsHPROFILE proof;
    cmsHTRANSFORM transform;
    cmsHTRANSFORM gamut;
};

void cuemol_cms_destroy(struct CueMolCms *value) {
    if (!value) return;
    if (value->transform) cmsDeleteTransform(value->transform);
    if (value->gamut) cmsDeleteTransform(value->gamut);
    if (value->proof) cmsCloseProfile(value->proof);
    if (value->context) cmsDeleteContext(value->context);
    free(value);
}

struct CueMolCms *cuemol_cms_create(const void *bytes, unsigned length, unsigned intent) {
    struct CueMolCms *value = calloc(1, sizeof(*value));
    if (!value) return NULL;
    value->context = cmsCreateContext(NULL, NULL);
    if (!value->context) { cuemol_cms_destroy(value); return NULL; }
    value->proof = cmsOpenProfileFromMemTHR(value->context, bytes, length);
    cmsHPROFILE srgb = cmsCreate_sRGBProfileTHR(value->context);
    if (value->proof && srgb) {
        value->transform = cmsCreateProofingTransformTHR(value->context,
            srgb, TYPE_RGB_8, srgb, TYPE_RGB_8, value->proof,
            intent, INTENT_ABSOLUTE_COLORIMETRIC, cmsFLAGS_SOFTPROOFING | cmsFLAGS_NOCACHE);
        value->gamut = cmsCreateProofingTransformTHR(value->context,
            srgb, TYPE_RGB_8, srgb, TYPE_RGB_8, value->proof, intent,
            INTENT_ABSOLUTE_COLORIMETRIC, cmsFLAGS_SOFTPROOFING | cmsFLAGS_GAMUTCHECK | cmsFLAGS_NOCACHE);
    }
    if (srgb) cmsCloseProfile(srgb);
    if (!value->transform || !value->gamut) { cuemol_cms_destroy(value); return NULL; }
    return value;
}

unsigned cuemol_cms_color(struct CueMolCms *value, unsigned rgb, int gamut) {
    unsigned char input[3] = { (rgb >> 16) & 255, (rgb >> 8) & 255, rgb & 255 };
    unsigned char output[3];
    if (gamut) {
        cmsUInt16Number alarm[cmsMAXCHANNELS] = { 0 };
        alarm[0] = input[0] == 255 && input[1] == 0 && input[2] == 255 ? 0 : 65535;
        alarm[1] = alarm[0] ? 0 : 65535;
        alarm[2] = 65535;
        cmsSetAlarmCodesTHR(value->context, alarm);
        cmsDoTransform(value->gamut, input, output, 1);
        return output[0] != (alarm[0] >> 8) || output[1] != (alarm[1] >> 8) || output[2] != 255;
    }
    cmsDoTransform(value->transform, input, output, 1);
    return (output[0] << 16) | (output[1] << 8) | output[2];
}

unsigned cuemol_cms_info(struct CueMolCms *value, unsigned type, char *text, unsigned length) {
    return cmsGetProfileInfoASCII(value->proof, (cmsInfoType)type, "en", "US", text, length);
}
