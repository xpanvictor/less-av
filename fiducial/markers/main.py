import cv2 as cv
import numpy as np

cap = cv.VideoCapture(0)
aruco_dict = cv.aruco.getPredefinedDictionary(cv.aruco.DICT_4X4_50)
aruco_params = cv.aruco.DetectorParameters()
detector = cv.aruco.ArucoDetector(aruco_dict, aruco_params)

while True:
    ret, frame = cap.read()
    if not ret:
        print("Failed to grab frame")
        break

    gray = cv.cvtColor(frame, cv.COLOR_BGR2GRAY)

    marker_corners, marker_ids, rejected_candidates = detector.detectMarkers(gray)
    if marker_ids is not None:
        cv.aruco.drawDetectedMarkers(frame, marker_corners, marker_ids)
        print(f"Detected markers: {marker_ids.flatten()}")

    cv.imshow("Robot ArUco Tracker", frame)
    if cv.waitKey(1) & 0xFF == ord("q"):
        break

cap.release()
cv.destroyAllWindows()
